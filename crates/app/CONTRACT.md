# crate: divebridge-app

**Role:** The application shell. Becomes the Tauri v2 backend exposing commands to
the SvelteKit UI and wiring the other crates:

ingest-(file|ble) -> store -> classify (rules) -> user selects Tracked -> enrich ->
ssi-api (fallback ssi-browser) -> store (record sync).

## Intended (later)
- Tauri commands: `list_dives`, `import_file`, `scan_devices`, `download_device`,
  `set_tracking`, `merge_dives`, `enrich_dive`, `upload_dives`, `auth_ssi`.
- Credential/token storage in OS keychain (decided in Round 2 grilling).
- macOS bundle entitlements for BLE.

Currently a placeholder binary (`divebridge`) so the workspace has a runnable
entrypoint.

## Dependencies
divebridge-core now; tauri + all crates when wired.
