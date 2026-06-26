# Decisions log (locked via spec grilling)

## Architecture (approved plan)
- Rust + Tauri v2 desktop app, SvelteKit UI, macOS, single-user/local. Name:
  **DiveBridge** (workspace + OS folder both `divebridge`).
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

## SSI mapping (locked from Spike 1 + Ken)
- **SSI `dive_nr` ≠ computer dive number.** It's the SSI logbook sequence and will
  almost never match the Perdix number (esp. a new computer). Map: compute next SSI
  number via read-back (`a21.php get_divelog` max+1) or let the user set it; keep
  `computer_dive_number` only in provenance.
- **Test-dive safety:** never set `log_linked_facility_id` / dive-center fields on
  test submissions, so no affiliated center is notified.
- **Auth for create = session cookie** (likely `PHPSESSID`) on my.divessi.com; SSO
  login (`rest.divessi.com/sso/login`) uses an `x-ssi-auth` header. For early
  testing, use a **session-cookie handoff** (Ken pastes the `cookie:` header into a
  scratchpad file) rather than implementing/storing login.
- **Dropdown vocabularies not in the HAR** (only selected values captured). Get full
  `var_*` option tables by fetching `mydivelog/add` with a live session, or from a
  mobile-app capture. See docs/api-capture.md.

## VALIDATED (live submit 2026-06-26)
- End-to-end proven: UDDF -> core::Dive -> SSI form -> live POST created dive #45.
- **Dive site is required** for a successful create (only required form field).
- Success response is indistinguishable from silent rejection (both 200 + redirect)
  -> ALWAYS verify by read-back of `/mydivelog` (`show/{nr}_{id}_{user}`).
- `dive_nr` = max+1 auto (Ken approved next-integer). To wire as a feature.
- PHPSESSID session is sufficient auth for create (no CSRF).

## Pending (Round 2 — remaining)
Credential/token storage location (post-testing), sync idempotency on re-upload
(update vs new), batch-failure UX, and whether to attach the dive-computer profile
(`diveComputerData_ue`) — pending a mobile-app capture.

## Pending core revisions (deliberate, not ad-hoc)
- **Tank begin/end pressure has no home.** Spike 3 found Shearwater `<tankdata>`
  carries `tankpressurebegin/end` (Pa→bar) but `core` has nowhere to put them, and
  these waypoints have no per-sample pressure → values currently dropped. SSI needs
  `pressure_start_bar`/`pressure_end_bar`, so add a `TankData`/begin-end field to
  `SourceRecording` (and surface in `DiveSummary`). Do as one focused core change
  before `ssi-api` mapping. (`pa_to_bar` already implemented in ingest-file.)
- CCR setpoint derivation from `divemode`/`calculatedpo2` (Sample.setpoint).
- Surface-event splitting (depth < 0.5 m for > 20 s) — one segment per dive today.
