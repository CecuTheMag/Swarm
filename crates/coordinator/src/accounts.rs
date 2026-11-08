//! Accounts (spec 0010): signing in and out, invitations (the only way to get an
//! account), and the platform admin's view of accounts.

use crate::{
    api::AppError,
    auth::{self, MIN_PASSWORD_LEN, PUBLIC_NETWORK},
    db, networks,
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use protocol::api::{
    AcceptInvite, AccountView, CreateInvite, CreatedInvite, InviteInfo, Login, Me, Role,
};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

fn me(account: &auth::Account) -> Me {
    Me {
        id: account.id,
        email: account.email.clone(),
        name: account.name.clone(),
        is_admin: account.is_admin,
    }
}

fn normalize_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_lowercase();
    let valid = email.len() <= 254
        && email
            .split_once('@')
            .is_some_and(|(user, domain)| !user.is_empty() && domain.contains('.'));
    if valid {
        Ok(email)
    } else {
        Err(AppError::BadRequest("a valid email address".into()))
    }
}

fn check_password(password: &str) -> Result<(), AppError> {
    if password.chars().count() < MIN_PASSWORD_LEN || password.len() > 1024 {
        return Err(AppError::BadRequest(format!(
            "a password of at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    Ok(())
}

/// Starts a session for `account_id` and returns the cookie that carries it.
async fn start_session(
    state: &AppState,
    account_id: Uuid,
    headers: &HeaderMap,
) -> Result<HeaderValue, AppError> {
    let token = auth::random_token();
    sqlx::query(
        "INSERT INTO sessions (token_hash, account_id, expires_at)
         VALUES ($1, $2, now() + make_interval(days => $3))",
    )
    .bind(auth::token_hash(&token))
    .bind(account_id)
    .bind(auth::SESSION_DAYS as i32)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE accounts SET last_login_at = now() WHERE id = $1")
        .bind(account_id)
        .execute(&state.db)
        .await?;
    // Expired sessions are cleared now and then, here rather than by a timer.
    if fastrand::u8(..) == 0 {
        sqlx::query("DELETE FROM sessions WHERE expires_at < now()")
            .execute(&state.db)
            .await?;
    }
    Ok(HeaderValue::from_str(&auth::session_cookie(&token, headers)).expect("ASCII cookie"))
}

/// `POST /api/auth/login`.
pub async fn login(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Json(body): Json<Login>,
) -> Result<Response, AppError> {
    if !auth::same_origin(&method, &headers) {
        return Err(AppError::Forbidden);
    }
    let email = body.email.trim().to_lowercase();
    if state.logins.blocked(&email) {
        return Err(AppError::TooManyRequests);
    }
    let row: Option<(Uuid, String, String, String, bool)> = sqlx::query_as(
        "SELECT id, email, name, password_hash, is_admin FROM accounts WHERE lower(email) = $1",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let hash = row.as_ref().map(|r| r.3.clone());
    let valid = auth::check_sign_in(&state, body.password.clone(), hash).await?;
    let Some((id, email_db, name, _, is_admin)) = row.filter(|_| valid) else {
        state.logins.failed(&email);
        return Err(AppError::BadRequest("wrong email or password".into()));
    };
    state.logins.succeeded(&email);
    let cookie = start_session(&state, id, &headers).await?;
    let account = auth::Account {
        id,
        email: email_db,
        name,
        is_admin,
    };
    tracing::info!(account_id = %id, "signed in");
    Ok(([(header::SET_COOKIE, cookie)], Json(me(&account))).into_response())
}

/// `POST /api/auth/logout`: ends this session.
pub async fn logout(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if !auth::same_origin(&method, &headers) {
        return Err(AppError::Forbidden);
    }
    if let Some(token) = auth::cookie(&headers, auth::SESSION_COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(auth::token_hash(&token))
            .execute(&state.db)
            .await?;
    }
    let clear = HeaderValue::from_str(&auth::clear_session_cookie()).expect("ASCII cookie");
    Ok(([(header::SET_COOKIE, clear)], StatusCode::NO_CONTENT).into_response())
}

/// `GET /api/me`: the signed-in account, or 401.
pub async fn current(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Result<Json<Me>, AppError> {
    let account = auth::require_account(&state, &method, &headers).await?;
    Ok(Json(me(&account)))
}

struct InviteRow {
    email: Option<String>,
    network_id: Option<Uuid>,
    role: Option<String>,
    network_name: Option<String>,
    invited_by: Option<String>,
}

/// Email, network, role, network name and inviter's name.
type InviteTuple = (
    Option<String>,
    Option<Uuid>,
    Option<String>,
    Option<String>,
    Option<String>,
);

async fn open_invite(db: &PgPool, token: &str) -> Result<InviteRow, AppError> {
    let row: Option<InviteTuple> = sqlx::query_as(
        "SELECT i.email, i.network_id, i.role, n.name, a.name FROM invites i
         LEFT JOIN networks n ON n.id = i.network_id
         LEFT JOIN accounts a ON a.id = i.created_by
         WHERE i.token_hash = $1 AND i.used_at IS NULL AND i.expires_at > now()",
    )
    .bind(auth::token_hash(token))
    .fetch_optional(db)
    .await?;
    let (email, network_id, role, network_name, invited_by) = row.ok_or(AppError::NotFound)?;
    Ok(InviteRow {
        email,
        network_id,
        role,
        network_name,
        invited_by,
    })
}

/// `GET /api/invites/{token}`: what the invitation offers.
pub async fn invite_info(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InviteInfo>, AppError> {
    let invite = open_invite(&state.db, &token).await?;
    Ok(Json(InviteInfo {
        email: invite.email,
        network_name: invite.network_name,
        role: invite.role.as_deref().and_then(Role::parse),
        invited_by: invite.invited_by,
    }))
}

/// `POST /api/invites/{token}`: a signed-in account joins the invitation's network;
/// anyone else creates an account (and is signed in).
pub async fn accept_invite(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(token): Path<String>,
    Json(body): Json<AcceptInvite>,
) -> Result<Response, AppError> {
    if !auth::same_origin(&method, &headers) {
        return Err(AppError::Forbidden);
    }
    let invite = open_invite(&state.db, &token).await?;
    let signed_in = auth::session_account(&state, &headers).await?;
    let mut tx = state.db.begin().await?;
    // Marking it used first makes a second, concurrent use find nothing to update.
    let claimed = sqlx::query(
        "UPDATE invites SET used_at = now() WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(auth::token_hash(&token))
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let (account_id, new_account) = match &signed_in {
        Some(account) => (account.id, false),
        None => {
            let email = normalize_email(if body.email.trim().is_empty() {
                invite.email.as_deref().unwrap_or_default()
            } else {
                &body.email
            })?;
            let name = body.name.trim();
            if name.is_empty() || name.chars().count() > 80 {
                return Err(AppError::BadRequest("a name of 1 to 80 characters".into()));
            }
            check_password(&body.password)?;
            let taken: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM accounts WHERE lower(email) = $1)",
            )
            .bind(&email)
            .fetch_one(&mut *tx)
            .await?;
            if taken {
                return Err(AppError::Conflict(
                    "an account with this email exists; sign in and open the link again".into(),
                ));
            }
            let password = body.password.clone();
            let hash = tokio::task::spawn_blocking(move || auth::hash_password(&password))
                .await
                .map_err(|_| AppError::BadRequest("could not set the password".into()))?;
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO accounts (id, email, name, password_hash) VALUES ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(&email)
            .bind(name)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
            (id, true)
        }
    };
    sqlx::query("UPDATE invites SET used_by = $2 WHERE token_hash = $1")
        .bind(auth::token_hash(&token))
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
    if let (Some(network_id), Some(role)) = (invite.network_id, invite.role.as_deref()) {
        // Joining keeps a higher role the account may already have.
        sqlx::query(
            "INSERT INTO memberships (network_id, account_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (network_id, account_id) DO UPDATE SET role = CASE
                 WHEN memberships.role = 'owner' OR EXCLUDED.role = 'viewer' AND memberships.role = 'admin'
                 THEN memberships.role ELSE EXCLUDED.role END",
        )
        .bind(network_id)
        .bind(account_id)
        .bind(role)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    tracing::info!(%account_id, new_account, "invitation accepted");
    let mut response = Json(serde_json::json!({ "network_id": invite.network_id })).into_response();
    if new_account {
        let cookie = start_session(&state, account_id, &headers).await?;
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    Ok(response)
}

pub(crate) async fn require_admin(
    state: &AppState,
    method: &Method,
    headers: &HeaderMap,
) -> Result<auth::Account, AppError> {
    let account = auth::require_account(state, method, headers).await?;
    if !account.is_admin {
        return Err(AppError::Forbidden);
    }
    Ok(account)
}

/// `POST /api/admin/invites` (platform admin): an invitation to create an account.
pub async fn admin_invite(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Json(body): Json<CreateInvite>,
) -> Result<(StatusCode, Json<CreatedInvite>), AppError> {
    let admin = require_admin(&state, &method, &headers).await?;
    let created =
        networks::create_invite(&state, body.email.as_deref(), None, Some(admin.id)).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// `GET /api/admin/accounts` (platform admin).
pub async fn admin_accounts(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Result<Json<Vec<AccountView>>, AppError> {
    require_admin(&state, &method, &headers).await?;
    let rows: Vec<(
        Uuid,
        String,
        String,
        bool,
        OffsetDateTime,
        Option<OffsetDateTime>,
    )> = sqlx::query_as(
        "SELECT id, email, name, is_admin, created_at, last_login_at FROM accounts
             ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, email, name, is_admin, created_at, last_login_at)| AccountView {
                    id,
                    email,
                    name,
                    is_admin,
                    created_at: db::unix_ms(created_at),
                    last_login_at: last_login_at.map(db::unix_ms),
                },
            )
            .collect(),
    ))
}

/// Creates or updates a platform admin with `password`, owner of the public network.
/// For `coordinator admin-create` and `SWARM_BOOTSTRAP_ADMIN` (development and tests).
pub async fn ensure_admin(db: &PgPool, email: &str, password: &str) -> anyhow::Result<Uuid> {
    let email = normalize_email(email).map_err(|e| anyhow::anyhow!("{e}"))?;
    let password = password.to_owned();
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&password)).await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (id, email, name, password_hash, is_admin)
         VALUES ($1, $2, $3, $4, true)
         ON CONFLICT ((lower(email))) DO UPDATE SET password_hash = EXCLUDED.password_hash, is_admin = true
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(&email)
    .bind(email.split('@').next().unwrap_or("admin"))
    .bind(hash)
    .fetch_one(db)
    .await?;
    sqlx::query(
        "INSERT INTO memberships (network_id, account_id, role) VALUES ($1, $2, 'owner')
         ON CONFLICT (network_id, account_id) DO UPDATE SET role = 'owner'",
    )
    .bind(PUBLIC_NETWORK)
    .bind(id)
    .execute(db)
    .await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_and_passwords_are_checked() {
        assert_eq!(normalize_email(" Ana@School.BG ").unwrap(), "ana@school.bg");
        assert!(normalize_email("ana").is_err());
        assert!(normalize_email("@school.bg").is_err());
        assert!(normalize_email("ana@school").is_err());
        assert!(check_password("short").is_err());
        assert!(check_password("long enough pass").is_ok());
    }
}
