# crate: divebridge-ssi-api

**Role:** Primary upload path — replicate SSI's app API directly over HTTP.

## Known surface (from research; see docs/ssi-integration.md)
- Base: `https://api.divessi.com/app/a21.php`, client id `ssiapp=0815_ADR`.
- Auth: GET `?l=<email>&p=<pass>&what=authenticate&ssiapp=0815_ADR` -> `{"token":…}`
  (HTTP 200 even on failure; detect missing token).
- Read: `?what=get_divelog&token=…&ssiapp=0815_ADR`. Fields `odin_user_log_*`.
- **Create: UNKNOWN** — discovered in Spike 1 (browser-first capture). Fill in here.

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
