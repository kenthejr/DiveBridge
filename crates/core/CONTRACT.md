# crate: divebridge-core

**Role:** The frozen, IO-free domain model. Everything else depends on this; do
not add IO, networking, or device code here.

## Public types (see `src/model.rs`, `src/rules.rs`, `src/tracking.rs`)

- `Dive` — top-level aggregate. Owns `sources: Vec<SourceRecording>` (covers both
  time-merge and cross-device multiplicity), a `primary_source`, a derived
  `DiveSummary`, an editable `DiveLog` overlay, `SyncState`, optional
  `RawVerification`.
  - `Dive::primary() -> Option<&SourceRecording>`
  - `Dive::content_hash() -> String` (sha256 of tracking+summary+log; for sync
    staleness)
  - `Dive::is_uploadable() -> bool` (true only for `TrackingKind::Tracked`)
- `SourceRecording` — immutable raw layer (one device, one import). Has
  `dedup_key()` = `serial::dive_number` or content hash fallback.
- `Segment` — one submersion (profile slice). A recording may hold several,
  split by surface intervals.
- `Sample`, `GasMix` (`AIR`, `n2_percent`, `is_air`), `DiveSite`, `GpsPoint`,
  `ArtifactRef`, `DeviceId`, `SourceKind`.
- `DiveLog`, `DiveSummary`, `Weather`, `Ocean`, `SummaryOverrides`.
- `SyncState` / `SyncRecord` / `SyncStatus`.
- `TrackingKind { Tracked (default), Training }`.
- Rules: `RuleMatcher`, `ClassificationRule`, `RuleContext`, `classify()`.
- Units (SI newtypes): `Meters, Celsius, Bar, Kilograms, Liters, Seconds`.

## Invariants

- All quantities are SI internally. Convert at the edges only.
- Raw layer (`SourceRecording` + `original_artifact`) is never mutated; edits live
  in `DiveLog`.
- Only `Tracked` dives are eligible for upload.
- Identity (`DiveId`) is stable across merges and re-syncs.

## Dependencies
serde, serde_json, chrono, sha2, hex, thiserror. No async, no IO.
