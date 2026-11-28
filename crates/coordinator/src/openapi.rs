//! The REST API's OpenAPI description (spec 0019), built from the handlers' own
//! annotations and the `protocol` types, served at `/api/openapi.json`. The web app's
//! API reference (`/docs/api`) renders it with Swagger UI.

use crate::{accounts, api, catalog, chaos, edits, models, networks, schools, ws};
use axum::{
    http::header,
    response::{IntoResponse, Response},
};
use std::sync::LazyLock;
use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Рояк API",
        description = "The coordinator's REST API: accounts, networks and their devices, \
            jobs, programs and school timetables. Devices and live views use the WebSocket \
            endpoints at the end.\n\n\
            Sign in with `POST /api/auth/login` (a session cookie), or send a network's API \
            key as `Authorization: Bearer <key>`. A request that changes data must come from \
            this site: its `Origin` must match the host. A network the caller cannot see \
            answers 404, so its existence does not leak.",
        license(name = "GPL-3.0-only", identifier = "GPL-3.0-only")
    ),
    paths(
        api::healthz,
        accounts::login,
        accounts::logout,
        accounts::current,
        accounts::invite_info,
        accounts::accept_invite,
        accounts::admin_invite,
        accounts::admin_accounts,
        networks::join_info,
        networks::list,
        networks::create,
        networks::get,
        networks::update,
        networks::delete,
        networks::devices,
        networks::update_device,
        networks::remove_device,
        networks::members,
        networks::update_member,
        networks::remove_member,
        networks::invite,
        networks::api_keys,
        networks::create_api_key,
        networks::revoke_api_key,
        networks::jobs,
        networks::create_job,
        networks::get_job,
        networks::cancel_job,
        networks::accept_unchecked,
        networks::job_image,
        networks::leaderboard,
        networks::model,
        catalog::outputs,
        catalog::list,
        catalog::create,
        catalog::ask_review,
        catalog::approved,
        catalog::review_queue,
        catalog::decision,
        schools::list,
        schools::create,
        schools::get,
        schools::save,
        schools::delete,
        api::example_school,
        edits::get,
        edits::save,
        edits::drop,
        api::create_job,
        api::list_jobs,
        api::get_job,
        api::cancel_job,
        api::job_image,
        chaos::kill_worker,
        chaos::cut_off_worker,
        chaos::add_cheaters,
        chaos::network_kill,
        chaos::network_cut_off,
        chaos::network_cheaters,
        catalog::module_file,
        catalog::blob_file,
        models::file,
        ws::worker,
        ws::dashboard,
        ws::network_live,
        ws::join_summary,
    ),
    modifiers(&Security, &Summaries),
    tags(
        (name = "health", description = "Whether the coordinator answers"),
        (name = "accounts", description = "Signing in and invitations"),
        (name = "networks", description = "A school's or team's own network of devices"),
        (name = "devices", description = "The devices in a network"),
        (name = "members", description = "Who may manage a network, and with what role"),
        (name = "api keys", description = "Keys for scripts; a key acts as an admin of its network"),
        (name = "jobs", description = "Jobs of a network: start, follow, cancel, results"),
        (name = "programs", description = "A network's own WebAssembly programs, reviewed before the public network runs them"),
        (name = "schools", description = "School timetables: the data, and editing a found timetable"),
        (name = "public jobs", description = "Jobs of the public volunteer network"),
        (name = "chaos", description = "Resilience demo: switch devices off, cut them off, add cheaters"),
        (name = "platform admin", description = "Accounts and program review, for the platform's admins"),
        (name = "files", description = "Code and data devices download, checked by their SHA-256"),
        (name = "websocket", description = "Upgrades to WebSocket: devices and live views"),
    )
)]
pub struct ApiDoc;

/// The two ways to identify: the session cookie of `POST /api/auth/login`, or an API key.
struct Security;

impl Modify for Security {
    fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
        let components = doc.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                crate::auth::SESSION_COOKIE,
                "Set by POST /api/auth/login",
            ))),
        );
        components.add_security_scheme(
            "api_key",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .description(Some(
                        "A network's API key (Мрежа → Ключове), or the deployment's submit \
                         token for the public network",
                    ))
                    .build(),
            ),
        );
    }
}

/// Handlers' doc comments often start with their own route ("`GET /api/me`: the
/// signed-in account"), which Swagger UI already shows: the summary keeps the words.
struct Summaries;

impl Modify for Summaries {
    fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
        for item in doc.paths.paths.values_mut() {
            let ops = [
                &mut item.get,
                &mut item.post,
                &mut item.put,
                &mut item.patch,
                &mut item.delete,
            ];
            for op in ops.into_iter().flatten() {
                op.summary = op.summary.take().and_then(|s| without_route(&s));
            }
        }
    }
}

/// "`POST /api/x`: does y." → "Does y."; a summary that is only the route goes.
fn without_route(summary: &str) -> Option<String> {
    let rest = match summary.strip_prefix('`').and_then(|s| s.split_once('`')) {
        Some((_, rest)) => rest.trim_start_matches([':', '.', ',', ' ']),
        None => summary,
    };
    let mut chars = rest.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

static SPEC: LazyLock<String> = LazyLock::new(|| {
    ApiDoc::openapi()
        .to_pretty_json()
        .expect("the OpenAPI description serialises")
});

/// `GET /api/openapi.json`.
pub async fn spec() -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        SPEC.as_str(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// `(METHOD path)` of every route in `router()`, read from its source: the router
    /// cannot list its routes, and a route added without documentation must fail here.
    fn routed() -> BTreeSet<String> {
        let src = include_str!("lib.rs");
        let start = src.find("pub fn router(").expect("router() exists");
        let src = &src[start..];
        let mut out = BTreeSet::new();
        let mut rest = src;
        while let Some(i) = rest.find(".route(") {
            rest = &rest[i + ".route(".len()..];
            // The call's text, up to its closing parenthesis.
            let mut depth = 1;
            let end = rest
                .char_indices()
                .find(|&(_, c)| {
                    match c {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                    depth == 0
                })
                .map(|(j, _)| j)
                .expect("balanced route call");
            let call = &rest[..end];
            let path = call.split('"').nth(1).expect("a path literal");
            let path = path.replace("{*", "{");
            for method in ["get", "post", "put", "patch", "delete"] {
                let called = call.match_indices(&format!("{method}(")).any(|(k, _)| {
                    k == 0
                        || !call.as_bytes()[k - 1].is_ascii_alphanumeric()
                            && call.as_bytes()[k - 1] != b'_'
                });
                if called {
                    out.insert(format!("{} {path}", method.to_uppercase()));
                }
            }
        }
        out
    }

    fn documented() -> BTreeSet<String> {
        let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let mut out = BTreeSet::new();
        for (path, item) in doc["paths"].as_object().unwrap() {
            for method in item.as_object().unwrap().keys() {
                if ["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                    out.insert(format!("{} {path}", method.to_uppercase()));
                }
            }
        }
        out
    }

    #[test]
    fn every_route_is_documented_and_nothing_else() {
        let routed = routed();
        let documented = documented();
        // The description itself is served but not described.
        let routed: BTreeSet<_> = routed
            .into_iter()
            .filter(|r| r != "GET /api/openapi.json")
            .collect();
        assert!(
            routed.len() > 60,
            "the source parser found {} routes",
            routed.len()
        );
        let missing: Vec<_> = routed.difference(&documented).collect();
        let extra: Vec<_> = documented.difference(&routed).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "not documented: {missing:?}; documented but not routed: {extra:?}"
        );
    }

    #[test]
    fn every_operation_has_a_tag_and_a_unique_id() {
        let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let mut ids = BTreeSet::new();
        for (path, item) in doc["paths"].as_object().unwrap() {
            for (method, op) in item.as_object().unwrap() {
                if !["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                    continue;
                }
                assert!(
                    op["tags"].as_array().is_some_and(|t| !t.is_empty()),
                    "{method} {path} has no tag"
                );
                // Client generators name their functions after it: it must be unique.
                let id = op["operationId"].as_str().expect("an operationId");
                assert!(ids.insert(id.to_owned()), "operationId {id} is used twice");
            }
        }
        let schemas = doc["components"]["schemas"].as_object().unwrap();
        for name in [
            "ApiError",
            "JobSummary",
            "CreateJob",
            "NetworkView",
            "SchoolRecord",
        ] {
            assert!(schemas.contains_key(name), "{name} is not in the schemas");
        }
    }

    #[test]
    fn summaries_lose_the_route_they_repeat() {
        assert_eq!(
            without_route("`GET /api/me`: the signed-in account, or 401.").as_deref(),
            Some("The signed-in account, or 401.")
        );
        assert_eq!(without_route("`POST /api/auth/login`."), None);
        assert_eq!(
            without_route("Stops a job.").as_deref(),
            Some("Stops a job.")
        );
    }

    #[test]
    fn spec_is_valid_json_with_both_security_schemes() {
        let doc: serde_json::Value = serde_json::from_str(&SPEC).unwrap();
        assert!(doc["openapi"].as_str().unwrap().starts_with("3."));
        let schemes = &doc["components"]["securitySchemes"];
        assert_eq!(schemes["session"]["in"], "cookie");
        assert_eq!(schemes["api_key"]["scheme"], "bearer");
    }
}
