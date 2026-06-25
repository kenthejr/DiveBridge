# Shearwater Cloud UDDF dialect notes

Derived from a real **Perdix 2** export (`crates/ingest-file/tests/fixtures/
perdix2-real.uddf`, Shearwater Cloud Desktop 2.12.9, UDDF 3.2.3) plus a CCR export
(`perdix-ccr-real.uddf`). These are the exact files the parser must handle.

- **Namespace:** `http://www.streit.cc/uddf/3.2/` (default ns; also xsi/xsd).
  `<uddf version="3.2.3">`. Parser must be namespace-aware (or strip ns).

## Units / encoding quirks (IMPORTANT — convert to SI core types)
- **Temperature: KELVIN.** `<temperature>281.15</temperature>`,
  `<airtemperature>275.15</airtemperature>` → `Celsius = K - 273.15`.
- **Tank pressure: PASCALS.** `<tankpressurebegin>1.949838E+07</tankpressurebegin>`
  → `Bar = Pa / 100_000` (1.95e7 Pa ≈ 195 bar). Scientific notation appears.
- **Surface/air pressure: PASCALS** (`<surfacepressure>98600</surfacepressure>`).
- **Depth: meters**, **divetime: seconds** (already SI).
- **Gas fractions:** `<o2>0.21</o2>`, `<he>0</he>` are 0–1 fractions → `*100` for
  `GasMix` percent.
- **visibility:** a STRING with unit, e.g. `<visibility>4m</visibility>` — parse
  number out.

## Structure → core mapping
- Device: `diver/owner/equipment/divecomputer` → `DeviceId{make:"Shearwater",
  model:<model>, serial:<serialnumber>}`. `model`="Perdix 2", `serialnumber`=
  "A3B6F031". `notes/para` carry FirmV/LogV (keep for provenance).
- `gasdefinitions/mix` (id like `CC1:21/00`, `OC1:21/00`) → `Vec<GasMix>`. Note CC*
  vs OC* names ~ closed/open circuit.
- `profiledata/repetitiongroup/dive`:
  - `informationbeforedive`: `divenumber` → `computer_dive_number`; `datetime` →
    segment/summary start; links to buddy/site/deco/profile.
  - `tankdata/tankpressurebegin|end` (Pa) → start/end pressure.
  - waypoints (`samples/waypoint`): `depth`(m), `divetime`(s, offset),
    `temperature`(K), `calculatedpo2`→`ppo2`, `cns`, `gradientfactor`,
    `switchmix ref` (gas switch → `gas_index`; only emitted on change),
    `divemode type` (opencircuit/closedcircuit → CCR detection / setpoint).
  - `informationafterdive`: `greatestdepth`(m)→max_depth, `averagedepth`(m),
    `diveduration`(s), `visibility`(string), `notes/para` (multi-paragraph →
    `DiveLog.notes` candidate), `observations` (ShearwaterDiveModeType).
- `divesite/site/geography/location` → `DiveSite.name` (e.g. "Ledi-Wracks"). No GPS
  in this export.
- `diver/buddy/personal/firstname` → `DiveLog.buddies`.

## Notes
- One real export = one dive with ~464 waypoints. Surface-event splitting into
  `Segment`s is a DiveBridge step (depth<0.5m>20s), not in the UDDF itself.
- The raw bytes are kept verbatim for the verifiable layer; this dialect mapping
  populates `SourceRecording` + a derived `DiveSummary`.
