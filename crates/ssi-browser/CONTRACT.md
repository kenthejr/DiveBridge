# crate: divebridge-ssi-browser

**Role:** Two jobs:
1. **Discovery (dev-time):** drive my.divessi.com via `chromiumoxide` (CDP) while a
   dive is logged manually, capturing network requests to find the create endpoint
   + field names. Feeds `divebridge-ssi-api` and docs/ssi-integration.md.
2. **Fallback (runtime):** if the direct API submit fails, replay the web form
   deterministically (recorded selectors/steps) to submit a dive.

## Intended API
- `capture_session() -> CapturedRequests` (dev tool)
- `submit_via_form(mapped_dive, session) -> RemoteId` (fallback)
- recorded form map cached on disk; no AI in steady state.

## Notes
- Prefer `chromiumoxide` (native CDP) over a Node+Playwright sidecar.
- Tauri webview cookies are NOT readable from Rust — auth via captured token /
  reqwest in ssi-api, not by scraping the embedded webview.

## Dependencies
divebridge-core, chromiumoxide (pending), thiserror.
