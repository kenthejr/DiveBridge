# DiveBridge

A self-contained Rust + Tauri desktop app that pulls dive logs off a Shearwater
Perdix 2, keeps a complete, verifiable local dive history (distinguishing
**training** from **tracked** dives), and uploads tracked dives to SSI (MySSI /
DiveSSI).

Status: **early scaffolding**. See the full spec & roadmap in the plan file
(`~/.claude/plans/i-want-to-build-curried-stroustrup.md`) and `docs/`.

## Workspace layout

```
crates/
  core         frozen domain model (Dive, Segment, SourceRecording, rules, sync)
  store        flat-file persistence (UDDF/JSON, content-hash dedup)   [stub]
  ingest-file  UDDF/XML/CSV -> core::Dive                              [stub]
  ingest-ble   Shearwater BLE -> core::Dive (via shearwater-ble crate) [stub]
  ssi-api      direct HTTP client for divessi.com                      [stub]
  ssi-browser  chromiumoxide/thirtyfour capture + fallback             [stub]
  enrich       Open-Meteo / NOAA data feeds (+ optional Claude CLI)     [stub]
  app          Tauri v2 shell                                          [stub]
ui/            SvelteKit frontend                                      [later]
```

The pure-Rust Shearwater protocol (`shearwater-protocol` + `shearwater-ble`) lives
in a **separate, dual-licensed public repo** so others can reuse it.

## Build

```sh
cargo check
cargo test
```
