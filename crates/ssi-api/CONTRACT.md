# crate: divebridge-ssi-api

**Role:** Primary upload path — replicate SSI's app API directly over HTTP.

## Confirmed surface (HAR capture 2026-06-25; see docs/ssi-integration.md)
- **Auth:** `POST https://rest.divessi.com/sso/login` JSON
  `{auth:"Portal", username, password}` → session cookies. (`SSI_LOGIN_URL`)
- **Create:** `POST .../code/process/mydivelog_18.php` form-urlencoded, ~88
  `odin_user_log_*` fields + `source=mydl_18_add_AddDiveOnline` + `submit=Submit`;
  session cookie + `user_master_id` required. (`SSI_CREATE_DIVE_URL`) Sanitized
  field fixture: `tests/fixtures/create-dive.request.txt`.
- **Validate dive nr:** `POST .../ajax_divelog_validate_dive_number.php`.
- **Dive-site search:** `GET .../code/geo/dive_site.json.sd.php?minlat&...&latitude&
  longitude`. (`SSI_DIVE_SITE_GEO_URL`)
- **Read-back (legacy):** `a21.php?what=get_divelog&token=…` for dedupe/verify.

Constants live in `src/lib.rs`. The constraints below (idempotency, token store)
still hold; auth is cookie-session (not token) for the create path.

## Intended API
- `SsiClient::new()` (reqwest + persistent cookie/token store)
- `authenticate(email, pass) -> Token`
- `get_divelog() -> Vec<SsiDive>` (read-back verification)
- `resolve_site(query) -> Vec<SsiSite>` (endpoint TBD)
- `create_dive(mapped) -> RemoteId` (Spike 1)
- mapping: `core::Dive` -> SSI `odin_user_log_*` fields (collapse multi-gas to
  primary gas + notes; SI values; resolve `ssi_site_id`)

## Invariants
- Idempotency via `core::Dive::content_hash()` + `SyncState`: skip already-synced
  unchanged dives.
- Token persisted to disk; never persist the password.

## Dependencies
divebridge-core, reqwest (+cookies/json), cookie_store, serde, serde_json, thiserror.
