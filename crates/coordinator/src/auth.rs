//! Who is asking (spec 0010): accounts signed in with a session cookie, API keys of a
//! network, and the deployment's submit token for the public network.
//!
//! Secrets are stored only as hashes: passwords with Argon2id, session tokens, invite
//! tokens and API keys with SHA-256 (they are 256-bit random, so a fast hash is enough).

use crate::{api::AppError, state::AppState};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::http::{HeaderMap, Method, header};
use protocol::api::Role;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// The public volunteer network, created by migration 0005.
pub const PUBLIC_NETWORK: Uuid = Uuid::from_u128(1);

pub const SESSION_COOKIE: &str = "swarm_session";
pub const SESSION_DAYS: i64 = 30;
pub const MIN_PASSWORD_LEN: usize = 10;

/// Prefix of API keys, so a leaked one is recognisable in logs and scanners.
pub const API_KEY_PREFIX: &str = "swk_";

/// 256 random bits from the operating system, as hex.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the OS has a random source");
    hex::encode(bytes)
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Argon2id with the crate's defaults (19 MiB, 2 passes): ~50 ms, so call it off the
/// async threads.
pub fn hash_password(password: &str) -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the OS has a random source");
    let salt = SaltString::encode_b64(&bytes).expect("16 bytes is a valid salt");
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hashing with valid parameters cannot fail")
        .to_string()
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

/// A hash to verify against when the account does not exist, so a wrong email takes
/// as long as a wrong password.
pub fn dummy_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| hash_password("not a real password"))
}

/// Sign-in password checks computed at once. Each takes 19 MiB and ~50 ms of a core,
/// and made-up emails never reach the per-email limit, so a flood of sign-ins queues
/// for these instead of running thousands of checks at once (spec 0016).
pub const SIGN_IN_CHECKS: usize = 4;

/// Checks `password` against the account's `hash` (the dummy one if there is no such
/// account) on a blocking thread, once one of the [`SIGN_IN_CHECKS`] is free. A sign-in
/// that waits longer than `sign_in_wait` is told to try again.
///
/// The check keeps its slot until it is done, even when the sign-in is given up (the
/// client hung up and the request was dropped): otherwise clients that hang up at once
/// would free slots while their checks still run, and the limit would not hold.
pub async fn check_sign_in(
    state: &AppState,
    password: String,
    hash: Option<String>,
) -> Result<bool, AppError> {
    let slot = state.sign_ins.clone().acquire_owned();
    let slot = tokio::time::timeout(state.config.sign_in_wait, slot)
        .await
        .map_err(|_| AppError::TooManyRequests)?
        .expect("the sign-in semaphore is never closed");
    let valid = tokio::task::spawn_blocking(move || {
        let _slot = slot;
        let hash = hash.unwrap_or_else(|| dummy_hash().to_owned());
        verify_password(&password, &hash)
    })
    .await;
    Ok(valid.unwrap_or(false))
}

/// The value of cookie `name`, if sent.
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
}

/// Whether the page that sent the request was served over HTTPS (behind the proxies the
/// coordinator itself only sees HTTP). Decides the cookie's `Secure` flag.
fn secure(headers: &HeaderMap) -> bool {
    headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|o| o.starts_with("https://"))
}

pub fn session_cookie(token: &str, headers: &HeaderMap) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_DAYS * 24 * 3600,
        if secure(headers) { "; Secure" } else { "" }
    )
}

pub fn clear_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

/// A request that changes something on a session's authority must come from this
/// site's own pages: its `Origin` must name the host it was sent to. SameSite cookies
/// already stop most cross-site requests; this closes the rest.
pub fn same_origin(method: &Method, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .and_then(|o| o.split_once("://").map(|(_, rest)| rest));
    matches!((origin, host), (Some(o), Some(h)) if o == h)
}

#[derive(Debug, Clone)]
pub struct Account {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
}

/// The account of the session cookie, if it is valid.
pub async fn session_account(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<Account>, AppError> {
    let Some(token) = cookie(headers, SESSION_COOKIE) else {
        return Ok(None);
    };
    let row: Option<(Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT a.id, a.email, a.name, a.is_admin FROM sessions s
         JOIN accounts a ON a.id = s.account_id
         WHERE s.token_hash = $1 AND s.expires_at > now()",
    )
    .bind(token_hash(&token))
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(id, email, name, is_admin)| Account {
        id,
        email,
        name,
        is_admin,
    }))
}

/// The signed-in account, or 401. For requests that change something, also checks
/// that they come from this site.
pub async fn require_account(
    state: &AppState,
    method: &Method,
    headers: &HeaderMap,
) -> Result<Account, AppError> {
    let account = session_account(state, headers)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !same_origin(method, headers) {
        return Err(AppError::Forbidden);
    }
    Ok(account)
}

/// How a request may act on a network.
#[derive(Debug, Clone)]
pub struct Access {
    pub network_id: Uuid,
    pub role: Role,
    /// None for API keys and the submit token.
    pub account: Option<Account>,
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Checks that the request may act on `network_id` with at least `min` role. API keys
/// (and the submit token for the public network) act as admins of their network;
/// accounts act with their membership's role. A network the caller cannot see is
/// "not found", so its existence does not leak.
pub async fn network_access(
    state: &AppState,
    method: &Method,
    headers: &HeaderMap,
    network_id: Uuid,
    min: Role,
) -> Result<Access, AppError> {
    // On a development machine that asks for it, the public network is open to anyone
    // who does not identify themselves, as before networks existed. Only on request: a
    // coordinator that merely lacks its token stays closed.
    if network_id == PUBLIC_NETWORK
        && state.config.open_public
        && state.config.submit_token.is_none()
        && bearer(headers).is_none()
        && cookie(headers, SESSION_COOKIE).is_none()
    {
        return Ok(Access {
            network_id,
            role: Role::Admin,
            account: None,
        });
    }
    if let Some(key) = bearer(headers) {
        let key_network = key_network(state, key)
            .await?
            .ok_or(AppError::Unauthorized)?;
        if key_network != network_id {
            return Err(AppError::NotFound);
        }
        if Role::Admin < min {
            return Err(AppError::Forbidden);
        }
        return Ok(Access {
            network_id,
            role: Role::Admin,
            account: None,
        });
    }
    let account = require_account(state, method, headers).await?;
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM memberships WHERE network_id = $1 AND account_id = $2",
    )
    .bind(network_id)
    .bind(account.id)
    .fetch_optional(&state.db)
    .await?;
    let role = role
        .and_then(|r| Role::parse(&r))
        .ok_or(AppError::NotFound)?;
    if role < min {
        return Err(AppError::Forbidden);
    }
    Ok(Access {
        network_id,
        role,
        account: Some(account),
    })
}

/// The network an API key (or the submit token) belongs to.
pub async fn key_network(state: &AppState, key: &str) -> Result<Option<Uuid>, AppError> {
    if let Some(token) = &state.config.submit_token {
        // Both sides hashed, so the comparison takes the same time whatever the guess.
        if Sha256::digest(key.as_bytes()) == Sha256::digest(token.as_bytes()) {
            return Ok(Some(PUBLIC_NETWORK));
        }
    }
    if !key.starts_with(API_KEY_PREFIX) {
        return Ok(None);
    }
    let network: Option<Uuid> = sqlx::query_scalar(
        "UPDATE api_keys SET last_used_at = now()
         WHERE token_hash = $1 AND revoked_at IS NULL RETURNING network_id",
    )
    .bind(token_hash(key))
    .fetch_optional(&state.db)
    .await?;
    Ok(network)
}

/// Failed sign-ins per email: after `MAX` within `WINDOW`, further attempts wait.
#[derive(Default)]
pub struct LoginLimiter {
    failures: std::sync::Mutex<std::collections::HashMap<String, (u32, Instant)>>,
}

impl LoginLimiter {
    const MAX: u32 = 8;
    const WINDOW: Duration = Duration::from_secs(15 * 60);

    pub fn blocked(&self, email: &str) -> bool {
        let map = self.failures.lock().unwrap();
        map.get(email)
            .is_some_and(|(n, since)| *n >= Self::MAX && since.elapsed() < Self::WINDOW)
    }

    pub fn failed(&self, email: &str) {
        let mut map = self.failures.lock().unwrap();
        let entry = map.entry(email.to_owned()).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= Self::WINDOW {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
        // Bounded memory under a flood of made-up emails.
        if map.len() > 10_000 {
            map.retain(|_, (_, since)| since.elapsed() < Self::WINDOW);
        }
    }

    pub fn succeeded(&self, email: &str) {
        self.failures.lock().unwrap().remove(email);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn passwords_hash_and_verify() {
        let hash = hash_password("correct horse battery");
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("correct horse batterY", &hash));
        assert!(!verify_password("anything", "not a hash"));
        // Salted: the same password hashes differently.
        assert_ne!(hash, hash_password("correct horse battery"));
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let (a, b) = (random_token(), random_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert_eq!(token_hash(&a), token_hash(&a));
        assert_ne!(token_hash(&a), token_hash(&b));
    }

    #[test]
    fn cookies_are_read_among_others() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; swarm_session=abc; b=2"),
        );
        assert_eq!(cookie(&h, SESSION_COOKIE).as_deref(), Some("abc"));
        assert_eq!(cookie(&h, "missing"), None);
    }

    #[test]
    fn changes_must_come_from_this_site() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, HeaderValue::from_static("swarm.example"));
        assert!(same_origin(&Method::GET, &h));
        assert!(!same_origin(&Method::POST, &h), "no Origin");
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(!same_origin(&Method::POST, &h));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://swarm.example"),
        );
        assert!(same_origin(&Method::DELETE, &h));
        assert!(session_cookie("t", &h).ends_with("; Secure"));
    }

    #[test]
    fn repeated_failures_block_sign_in_for_a_while() {
        let limiter = LoginLimiter::default();
        for _ in 0..LoginLimiter::MAX {
            assert!(!limiter.blocked("a@b"));
            limiter.failed("a@b");
        }
        assert!(limiter.blocked("a@b"));
        assert!(!limiter.blocked("c@d"));
        limiter.succeeded("a@b");
        assert!(!limiter.blocked("a@b"));
    }
}
