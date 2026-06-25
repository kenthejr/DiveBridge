# Decisions log (locked via spec grilling)

## Architecture (approved plan)
- Rust + Tauri v2 desktop app, SvelteKit UI, macOS, single-user/local. Name:
  **DiveBridge** (OS folder still `ssi-uploader`; rename deferred).
- Cargo workspace of small crates, each with a `CONTRACT.md`; `core` frozen first.
- Flat-file store (UDDF/JSON), content-hash dedup. No SQLite.
- Device: pure-Rust Shearwater protocol (no LGPL/FFI), published as a separate
  `shearwater-protocol`/`shearwater-ble` repo (dual MIT/Apache).
- SSI: direct HTTP submission primary; browser path for discovery + fallback.
- AI deferred & optional: deterministic data-feed enrichment now; optional,
  fail-safe local-Claude-CLI provider; no Anthropic API key.

## Domain (Round 1 / 1b grilling)
1. **Identity/dedup:** `SourceRecording::dedup_key()` = `serial::dive_number`, else
   content hash. Logical `DiveId` is stable across merges & re-syncs.
2. **Tracking:** `TrackingKind {Tracked(default), Training}`, mutually exclusive.
   Only `Tracked` dives upload to SSI.
3. **Raw vs editable:** immutable `SourceRecording` (+ verbatim artifact) vs
   editable `DiveLog` overlay. Raw layer is verifiable for insurance.
4. **Units:** SI internally; convert at edges.
5. **Lossless:** full multi-gas/CCR/per-sample data kept in `core`; SSI mapping
   collapses to primary gas + notes.
6. **Dive site:** `DiveSite{name, gps?, ssi_site_id?}`; resolve/cache SSI id at
   upload. Perdix has no GPS.
7. **Merge (multi-surface):** logical `Dive` holds `segments`/multiple
   `SourceRecording`s. Manual multi-select merge + auto-suggest on short surface
   intervals; **persisted** so device re-sync doesn't un-merge.
8. **Surface-event split:** depth < 0.5 m for > 20 s (configurable) splits segments.
9. **Classification rules (deterministic):** match on max-depth (pool), site name,
   GPS proximity (once a GPS source exists), time windows (later). Manual override
   always wins.
10. **Verifiability:** verbatim artifact + sha256 + **SSH-signed manifest**
    (`id_ed25519`) of the raw layer.
11. **GPS / multi-device:** `Dive.sources` is a list → covers cross-device (Perdix 2
    + Garmin Mk3i) and time-merge. One `primary_source` for the canonical profile.
    Garmin/Swift ingestion **deferred** behind the seam.

## Pending (Round 2 — after Spike 1)
SSI field mapping specifics, dive-site resolution endpoint, sync idempotency on
re-upload (update vs new), credential/token storage location, batch-failure UX.

## Pending core revisions (deliberate, not ad-hoc)
- **Tank begin/end pressure has no home.** Spike 3 found Shearwater `<tankdata>`
  carries `tankpressurebegin/end` (Pa→bar) but `core` has nowhere to put them, and
  these waypoints have no per-sample pressure → values currently dropped. SSI needs
  `pressure_start_bar`/`pressure_end_bar`, so add a `TankData`/begin-end field to
  `SourceRecording` (and surface in `DiveSummary`). Do as one focused core change
  before `ssi-api` mapping. (`pa_to_bar` already implemented in ingest-file.)
- CCR setpoint derivation from `divemode`/`calculatedpo2` (Sample.setpoint).
- Surface-event splitting (depth < 0.5 m for > 20 s) — one segment per dive today.
