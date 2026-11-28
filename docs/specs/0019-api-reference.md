# 0019 — The API reference (OpenAPI)

Status: implemented

## Problem

Everything the console does goes through the coordinator's REST API, and scripts use it
with a network's API key (spec 0010), but the API was described only in the code and in
scattered specs. A developer, a school's IT person or a jury could not see what it offers
without reading Rust.

## What there is

- `GET /api/openapi.json`: an OpenAPI 3.1 description of every route the coordinator
  serves, 66 operations on 51 paths, the WebSocket endpoints included (as `101` upgrades).
  It is built at start-up from the handlers' own `#[utoipa::path]` annotations and from
  the `protocol` crate's request and response types (`ToSchema`, behind the `openapi`
  feature), so the schemas are the types the code uses, with their doc comments.
- Two security schemes: the session cookie of `POST /api/auth/login`, and a network's
  API key as `Authorization: Bearer`. Every operation says which it takes and which
  errors it returns (`ApiError`).
- `/docs/api` in the web app: Swagger UI (`swagger-ui-dist`, pinned, bundled, so the
  site's content security policy holds) on a light sheet, with a short introduction in
  Bulgarian. „Try it out“ uses the browser's own session; a change still needs this site
  as its `Origin`, as from the console. The footer links to it.

## Why annotations and not a hand-written file

A hand-written description drifts from the code. Here the types are the code's, and a
test fails when the router and the description disagree.

## Verified by

- `openapi.rs` tests: every route in `router()` (read from its source) is documented and
  nothing else is; every operation has a tag and a unique `operationId`; the core schemas
  and both security schemes are present; route prefixes are removed from summaries.
  Removing one path from the description makes the first test name it.
- By hand: the page renders against a running coordinator, and „Try it out“ on
  `GET /api/jobs` returns the public jobs.
