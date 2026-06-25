# ingest-file test fixtures

## Real Shearwater exports (primary)
- `perdix2-real.uddf` — **real Shearwater Cloud Desktop export from a Perdix 2**
  (serial A3B6F031, dive #42, 2024-12-01). UDDF 3.2.3, ns `http://www.streit.cc/
  uddf/3.2/`, 464 waypoints (depth/divetime/temperature/calculatedpo2), 2 gas
  mixes, tankdata, buddy, dive site "Ledi-Wracks". This is the exact production
  dialect — the parser's primary target.
  Source: `SebastianThomas/std-dive-logger-backend` test resources.
- `perdix-ccr-real.uddf` — real Shearwater Cloud export, **closed-circuit (CCR)**
  dive (UDDF 3.2.1, 10 waypoints). Exercises `divemode=closedcircuit`.
  Source: `weppos/uddf-swift` `Tests/.../real/`.

See `docs/uddf-shearwater-dialect.md` for the unit quirks (Kelvin temps, Pascal
pressures, fractional gas) and element→`core` mapping.

## Thin reference (edge case)
- `go-uddf-valid.uddf` — summary-only, non-standard namespace, no profile. Keep as
  a degenerate/edge case only. Source: `Flipez/go-uddf`.

## Still wanted
Ken's own Perdix 2 export once the device is in hand, to confirm nothing in his
account/firmware differs. The files above are already real Perdix 2 / Shearwater
data, so the parser can be built and tested now.
