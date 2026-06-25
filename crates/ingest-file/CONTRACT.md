# crate: divebridge-ingest-file

**Role:** Turn exported files into `core::SourceRecording` / `core::Dive`.

## Intended API
- `parse_uddf(bytes: &[u8]) -> Result<Vec<SourceRecording>>` (UDDF 3.x; Shearwater
  Cloud export is the primary target)
- `parse_csv(...)`, `parse_subsurface_xml(...)` — later
- `to_dives(Vec<SourceRecording>) -> Vec<Dive>` (applies surface-event splitting
  into `Segment`s and builds `DiveSummary`)

## Notes
- Use `quick-xml` + serde (added in Spike 3 against a real export fixture).
- Preserve the original bytes for the verifiable raw layer (store handles writing;
  this crate surfaces them via `ArtifactRef` inputs).
- Surface-split: depth < 0.5 m for > 20 s splits a recording into segments
  (thresholds configurable; logic shared with ingest-ble — consider a small shared
  helper in core later).

## Dependencies
divebridge-core, quick-xml (pending), thiserror.
