# crate: divebridge-store

**Role:** Persist the full dive history as flat files on disk (no DB). Git-friendly,
Subsurface-like.

## Intended layout (under a configurable data dir)
```
data/
  dives/<dive_id>.json        # serialized core::Dive (editable overlay + summary)
  raw/<source_id>/...         # verbatim original artifacts (immutable)
  merges.json                 # persisted merge groupings (survives re-sync)
  rules.json                  # ClassificationRule list
  ledger.json                 # sync ledger (mirror of per-dive SyncState)
  manifest.sig                # SSH-signed manifest of the raw layer
```

## Intended API
- `Store::open(dir) -> Result<Store>`
- `list_dives() -> Vec<DiveId>` / `load(&DiveId) -> Result<Dive>` /
  `save(&Dive) -> Result<()>`
- `upsert_source(SourceRecording) -> dedup outcome` (uses
  `SourceRecording::dedup_key()`; idempotent re-import)
- `merge(ids: &[DiveId]) -> Result<DiveId>` / `unmerge(...)` — persisted
- `load_rules()/save_rules()`, ledger read/write
- Verifiability: write verbatim artifacts, compute manifest sha256, request an SSH
  signature (delegated; signing key from config).

## Invariants
- Raw artifacts written once, never modified.
- Dedup by `SourceRecording::dedup_key()`; merges remembered so device re-sync does
  not un-merge.

## Dependencies
divebridge-core, serde, serde_json, thiserror. (Add `fs2`/`tempfile` for atomic
writes when implemented.)
