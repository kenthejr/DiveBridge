# 🤿 DiveBridge

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg?logo=rust)
![Platform](https://img.shields.io/badge/platform-macOS-lightgrey.svg?logo=apple)
![Status](https://img.shields.io/badge/status-early%20development-yellow.svg)

**Pull dive logs off a Shearwater Perdix 2 and upload them straight to your SSI
(MySSI) logbook — without clicking through the web form.**

DiveBridge keeps a complete, verifiable local dive history, lets you mark dives as
**training** vs **tracked**, and submits the tracked ones to SSI over its
(undocumented) web API — discovered, mapped, and validated end-to-end against a
real account.

> ⚠️ **Early development.** The full UDDF → SSI pipeline works and is proven, but
> there's no GUI yet and direct Bluetooth sync is still in progress. It's a CLI for
> now.

---

## Why

SSI has no public API and no good desktop sync. Subsurface is powerful but clunky
for this workflow, and logging dives by hand in the MySSI web form is tedious.
DiveBridge automates the boring part: parse the dive, map it to SSI's fields, and
create the log directly — while keeping your own canonical, lossless history.

## What works today

| Capability | Status |
|---|---|
| Parse Shearwater Cloud **UDDF** exports → canonical model | ✅ (real Perdix 2 data) |
| Lossless domain model (multi-gas, CCR, segments, tanks, provenance) | ✅ frozen `core` |
| **Flat-file local store** (history, dedup, training/tracked, merges) | ✅ |
| Map a dive → SSI form + **submit directly over HTTP** | ✅ **validated live** |
| Dive-site **search/resolve by name** (geocode → SSI site id) | ✅ |
| Auto **dive number** (next in your logbook) | ✅ |
| Direct **Bluetooth** sync from the Perdix 2 (pure Rust) | 🔜 in progress |
| Tauri + SvelteKit **desktop GUI** | 🔜 planned |
| Data **enrichment** (weather / tide / ocean, tags) | 🔜 planned |

## How it works

```
Shearwater UDDF ─▶ core::Dive ─▶ flat-file store ─▶ SSI form mapping ─▶ POST to MySSI
   (or BLE 🔜)        (lossless)     (history,          (vocab, units,      (verified by
                                      training/tracked)   site resolve)        read-back)
```

The design principle: **deterministic Rust on the hot path; AI only for discovery
and (optional, fail-safe) enrichment** — no API key required to run the app.

## Architecture

A Cargo workspace of small crates, each with a `CONTRACT.md`:

| Crate | Role |
|---|---|
| `divebridge-core` | Frozen domain model: `Dive`, `Segment`, `SourceRecording`, `TankData`, rules, sync |
| `divebridge-store` | Flat-file persistence (JSON + verbatim raw artifacts, dedup, ledger) |
| `divebridge-ingest-file` | UDDF/XML → `core::Dive` (`quick-xml`) |
| `divebridge-ingest-ble` | Shearwater BLE → `core::Dive` (adapter; 🔜) |
| `divebridge-ssi-api` | SSI web-flow client + `Dive` → form mapping + site/geocode |
| `divebridge-ssi-browser` | Browser capture / deterministic-replay fallback (🔜) |
| `divebridge-enrich` | Data-feed enrichment + optional fail-safe AI provider (🔜) |
| `divebridge-app` | MVP CLI today; Tauri v2 backend later |

The pure-Rust Shearwater protocol will live in a **separate, dual-licensed repo**
(`shearwater-protocol` + `shearwater-ble`) so others can reuse it.

See [`docs/`](docs/) for the full spec, SSI integration notes, vocabularies, and
the capture/parallelization playbooks.

## Build

```sh
cargo build
cargo test --workspace
```

Requires stable Rust 1.90+ (`rust-toolchain.toml`). macOS-first.

## Usage (CLI)

```sh
# Summarize a UDDF export
divebridge inspect dive.uddf

# Build & print the exact SSI form body — no network
divebridge dry-run dive.uddf --user-master-id <ID> --site-name "Folsom Point"

# Search SSI dive sites near a coordinate
divebridge sites 38.70 -121.14

# Submit to SSI (guarded: requires --phpsessid AND --yes; never attaches a center)
divebridge submit dive.uddf --user-master-id <ID> \
  --site-name "Folsom Point" --bow fresh --water-body-id 16 --watertype-id 4 \
  --phpsessid <SESSION_COOKIE> --yes
```

`--dive-nr` is optional (auto-assigned as the next number in your logbook). A site
is required by SSI; `--site-name` geocodes and resolves it, or pass `--site-id`.

## Privacy & safety

- Dive data and credentials stay **local**. Nothing is committed or sent anywhere
  except your explicit upload to SSI.
- `submit` never runs without `--phpsessid` **and** `--yes`, and **never attaches a
  dive center** (so affiliated facilities aren't notified of test dives).
- The immutable raw layer is kept verbatim and content-hashed for verifiability.

## Roadmap

1. **Spike 2 — Bluetooth**: pure-Rust Shearwater protocol (`shearwater-protocol` /
   `shearwater-ble`), published as its own crate.
2. **Desktop GUI**: Tauri v2 + SvelteKit — login, connect/import, browse history,
   filter training vs tracked, multi-select upload.
3. **Enrichment**: weather/tide/ocean via free data feeds; optional local-AI tags.
4. **More devices**: Garmin (FIT) ingestion behind the multi-source seam.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: `cargo fmt`, `clippy -D warnings`,
tests green, keep `core` frozen, never commit captures or personal data.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution you submit
shall be dual licensed as above, without additional terms.
