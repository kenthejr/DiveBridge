# crate: divebridge-ingest-ble

**Role:** Thin adapter from the external `shearwater-ble` crate's parsed dives into
`core::SourceRecording` / `core::Dive`. No protocol logic here — that lives in the
separate public repo.

## Separate repo (Spike 2 seeds it)
- `shearwater-protocol` — sans-IO UDS codec for the Petrel family (Perdix 2).
  Pure, deterministic, fixture-tested. No BLE dep.
- `shearwater-ble` — `btleplug` transport over the protocol crate.
- Dual MIT/Apache-2.0.

## Intended API (this crate)
- `discover() -> Vec<DiveComputer>` (scan)
- `download(device) -> Result<Vec<SourceRecording>>` (maps shearwater dives,
  sets `DeviceId{make:"Shearwater",model,serial}`, `SourceKind::ShearwaterBle`,
  `computer_dive_number`)

## macOS notes
Needs `NSBluetoothAlwaysUsageDescription` + `com.apple.security.device.bluetooth`
in the Tauri bundle. Shearwater GATT: svc `fe25c237-…`, char `27b7570b-…`.

## Dependencies
divebridge-core, shearwater-ble (pending Spike 2), thiserror.
