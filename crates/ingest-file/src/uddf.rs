//! Shearwater Cloud UDDF (3.x) parser.
//!
//! Maps the Shearwater dialect (default namespace `http://www.streit.cc/uddf/3.2/`)
//! into the frozen `core` domain model. See `docs/uddf-shearwater-dialect.md` for
//! the element→core mapping and the unit quirks this module compensates for
//! (Kelvin temperatures, Pascal pressures, 0–1 gas fractions).
//!
//! The parser is a small streaming state machine over `quick-xml` events. We
//! match on *local* element names (the part after any `:` and ignoring the
//! namespace prefix), so it is tolerant of how the namespace is declared.

use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::core::{
    DeviceId, DiveSummary, GasMix, Sample, Segment, SourceId, SourceKind, SourceRecording,
    TankData, TrackingKind,
};
use crate::core::{Dive, DiveId, DiveLog, SyncState};
use crate::units::{Bar, Celsius, Meters, Seconds};
use crate::IngestError;

// --- unit conversion helpers -------------------------------------------------

/// Kelvin → degrees Celsius. UDDF temperatures are Kelvin in this dialect.
pub fn kelvin_to_c(kelvin: f64) -> f64 {
    kelvin - 273.15
}

/// Pascals → bar. UDDF tank/surface pressures are Pascals (1 bar = 100_000 Pa).
pub fn pa_to_bar(pascals: f64) -> f64 {
    pascals / 100_000.0
}

// --- intermediate, dialect-shaped structures ---------------------------------

/// A gas mix as it appears in `<gasdefinitions>`, keyed by its `id` attribute so
/// `<switchmix ref=...>` can resolve to an index into `gases`.
struct GasDef {
    id: String,
    mix: GasMix,
}

/// Accumulator for one `<dive>` element while we stream through its children.
#[derive(Default)]
struct DiveAcc {
    dive_number: Option<u32>,
    start: Option<DateTime<Utc>>,
    duration_s: Option<i64>,
    greatest_depth: Option<f64>,
    average_depth: Option<f64>,
    samples: Vec<Sample>,
    /// Real (non-zero) tank pressure blocks, already converted to bar.
    tanks: Vec<TankData>,
}

// --- public API --------------------------------------------------------------

/// Parse a Shearwater UDDF export into one `SourceRecording` per `<dive>`.
///
/// `imported_at` stamps every recording; callers pass a fixed timestamp for
/// determinism, or `Utc::now()`. (Tests must not assert on this field.)
pub fn parse_uddf_at(
    bytes: &[u8],
    imported_at: DateTime<Utc>,
) -> Result<Vec<SourceRecording>, IngestError> {
    let text = std::str::from_utf8(bytes)?;
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    // Device + gas tables are file-level; dives reference them.
    let mut device = DeviceFields::default();
    let mut gas_defs: Vec<GasDef> = Vec::new();
    let mut dives: Vec<DiveAcc> = Vec::new();

    // Cursor describing which region of the document we are inside. The dialect
    // reuses generic element names (e.g. `<name>`, `<datetime>`) in several
    // contexts, so we gate text capture on these coarse regions.
    let mut in_divecomputer = false;
    let mut in_gasdefinitions = false;
    let mut in_dive = false;
    let mut in_before = false;
    let mut in_after = false;

    // Per-mix scratch while inside a `<mix>`.
    let mut cur_mix: Option<MixScratch> = None;
    // Per-tankdata scratch while inside a `<tankdata>` block.
    let mut cur_tank: Option<TankScratch> = None;
    // Per-waypoint scratch while inside a `<waypoint>`.
    let mut cur_wp: Option<WaypointScratch> = None;
    // The active gas index, carried forward across waypoints — `<switchmix>` is
    // only emitted when the gas actually changes.
    let mut active_gas: Option<u16> = None;

    // The element whose text we are currently capturing, as a local name.
    let mut text_target: Option<String> = None;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(IngestError::Xml(e)),
            Ok(Event::Eof) => break,

            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "divecomputer" => in_divecomputer = true,
                    "gasdefinitions" => in_gasdefinitions = true,
                    "mix" if in_gasdefinitions => {
                        let id = attr_value(&e, "id").unwrap_or_default();
                        cur_mix = Some(MixScratch::new(id));
                    }
                    "dive" => {
                        in_dive = true;
                        dives.push(DiveAcc::default());
                        active_gas = None;
                    }
                    "informationbeforedive" => in_before = true,
                    "informationafterdive" => in_after = true,
                    "tankdata" if in_dive => cur_tank = Some(TankScratch::default()),
                    "waypoint" if in_dive => cur_wp = Some(WaypointScratch::default()),
                    // Leaf elements whose text content we want.
                    "model" | "serialnumber" | "o2" | "he" | "divenumber" | "datetime"
                    | "diveduration" | "greatestdepth" | "averagedepth" | "depth" | "divetime"
                    | "temperature" | "calculatedpo2" | "tankpressurebegin" | "tankpressureend" => {
                        text_target = Some(name);
                    }
                    // `<name>` is captured only as a device-computer model fallback.
                    "name" if in_divecomputer => text_target = Some(name),
                    _ => {}
                }
            }

            Ok(Event::Empty(e)) => {
                // Self-closing elements carry their data in attributes.
                let name = local_name(e.name().as_ref());
                if name == "switchmix" && in_dive {
                    if let Some(ref_id) = attr_value(&e, "ref") {
                        active_gas = gas_index_for(&gas_defs, &ref_id);
                    }
                }
            }

            Ok(Event::Text(t)) => {
                let Some(target) = text_target.as_deref() else {
                    continue;
                };
                let raw = t.unescape().map_err(IngestError::Xml)?;
                let text = raw.trim();
                apply_text(
                    target,
                    text,
                    in_divecomputer,
                    &mut device,
                    cur_mix.as_mut(),
                    in_before,
                    in_after,
                    dives.last_mut(),
                    cur_wp.as_mut(),
                    cur_tank.as_mut(),
                );
            }

            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "divecomputer" => in_divecomputer = false,
                    "gasdefinitions" => in_gasdefinitions = false,
                    "mix" => {
                        if let Some(m) = cur_mix.take() {
                            gas_defs.push(GasDef {
                                id: m.id,
                                mix: GasMix {
                                    o2_percent: m.o2 * 100.0,
                                    he_percent: m.he * 100.0,
                                },
                            });
                        }
                    }
                    "informationbeforedive" => in_before = false,
                    "informationafterdive" => in_after = false,
                    "tankdata" => {
                        // The Perdix emits ~6 tankdata blocks; only the populated
                        // ones are real. Skip all-zero (both begin and end == 0).
                        if let (Some(tank), Some(acc)) = (cur_tank.take(), dives.last_mut()) {
                            if let Some(data) = tank.into_tank_data() {
                                acc.tanks.push(data);
                            }
                        }
                    }
                    "dive" => in_dive = false,
                    "waypoint" => {
                        if let (Some(wp), Some(acc)) = (cur_wp.take(), dives.last_mut()) {
                            acc.samples.push(wp.into_sample(active_gas));
                        }
                    }
                    _ => {}
                }
                // Any leaf element's text capture ends when the element closes.
                if text_target.as_deref() == Some(name.as_str()) {
                    text_target = None;
                }
            }

            _ => {}
        }
        buf.clear();
    }

    let gases: Vec<GasMix> = gas_defs.iter().map(|g| g.mix).collect();
    let device_id = device.into_device_id();

    // Build one SourceRecording per dive.
    let mut out = Vec::with_capacity(dives.len());
    for acc in dives {
        let segment = build_segment(&acc)?;
        // dedup_key needs a stable id; derive a provisional one from serial +
        // dive number (or start) before the recording exists.
        let provisional = match acc.dive_number {
            Some(n) => format!("{}::{}", device_id.serial, n),
            None => format!("{}::{}", device_id.serial, segment.start.to_rfc3339()),
        };
        let rec = SourceRecording {
            id: SourceId(provisional),
            device: device_id.clone(),
            kind: SourceKind::UddfFile,
            imported_at,
            computer_dive_number: acc.dive_number,
            // TODO: the store writes verbatim bytes later and fills this in.
            original_artifact: None,
            gases: gases.clone(),
            tanks: acc.tanks.clone(),
            // TODO: surface-event splitting (depth < 0.5 m for > 20 s) will turn
            // this single segment into several; one Segment per <dive> for now.
            segments: vec![segment],
            gps_track: Vec::new(),
        };
        out.push(rec);
    }

    Ok(out)
}

/// Convenience wrapper using `Utc::now()` for `imported_at`.
pub fn parse_uddf(bytes: &[u8]) -> Result<Vec<SourceRecording>, IngestError> {
    parse_uddf_at(bytes, Utc::now())
}

/// Build a `Dive` per `SourceRecording`, deriving a `DiveSummary` from its
/// segments. (Surface-splitting into multiple segments is a future step;
/// `descent_count` therefore equals the segment count.)
pub fn to_dives(sources: Vec<SourceRecording>) -> Vec<Dive> {
    sources
        .into_iter()
        .map(|src| {
            let id = DiveId(src.dedup_key());
            let primary_source = src.id.clone();
            let summary = derive_summary(&src);
            Dive {
                id,
                tracking: TrackingKind::default(),
                primary_source,
                sources: vec![src],
                summary,
                log: DiveLog::default(),
                sync: SyncState::default(),
                verification: None,
            }
        })
        .collect()
}

// --- summary derivation ------------------------------------------------------

fn derive_summary(src: &SourceRecording) -> DiveSummary {
    let segments = &src.segments;
    let start = segments.first().map(|s| s.start).unwrap_or_else(Utc::now);

    let total_bottom_time = Seconds(segments.iter().map(|s| s.duration.0).sum());
    let max_depth = segments
        .iter()
        .map(|s| s.max_depth.0)
        .fold(0.0_f64, f64::max);

    // avg_depth: present only if every segment exposes one (mean of the values).
    let avg_depth = if segments.iter().all(|s| s.avg_depth.is_some()) && !segments.is_empty() {
        let sum: f64 = segments
            .iter()
            .filter_map(|s| s.avg_depth)
            .map(|d| d.0)
            .sum();
        Some(Meters(sum / segments.len() as f64))
    } else {
        None
    };

    let min_temp = segments
        .iter()
        .filter_map(|s| s.min_temp)
        .map(|c| c.0)
        .reduce(f64::min)
        .map(Celsius);

    // Surface the primary (first) tank's begin/end pressures for SSI mapping.
    let primary_tank = src.tanks.first();
    let pressure_start = primary_tank.and_then(|t| t.pressure_begin);
    let pressure_end = primary_tank.and_then(|t| t.pressure_end);

    DiveSummary {
        start,
        // total_runtime == bottom time for now (no surface intervals split yet).
        total_runtime: total_bottom_time,
        total_bottom_time,
        max_depth: Meters(max_depth),
        avg_depth,
        descent_count: segments.len() as u32,
        min_temp,
        gases: src.gases.clone(),
        pressure_start,
        pressure_end,
    }
}

fn build_segment(acc: &DiveAcc) -> Result<Segment, IngestError> {
    let start = acc
        .start
        .ok_or_else(|| IngestError::Missing("dive datetime (informationbeforedive)".into()))?;
    let duration = Seconds(acc.duration_s.unwrap_or(0));
    let max_depth = Meters(acc.greatest_depth.unwrap_or(0.0));
    let avg_depth = acc.average_depth.map(Meters);
    // Min temperature across waypoints (already Celsius on the Sample).
    let min_temp = acc
        .samples
        .iter()
        .filter_map(|s| s.temperature)
        .map(|c| c.0)
        .reduce(f64::min)
        .map(Celsius);

    Ok(Segment {
        start,
        duration,
        max_depth,
        avg_depth,
        min_temp,
        samples: acc.samples.clone(),
    })
}

// --- scratch types -----------------------------------------------------------

struct MixScratch {
    id: String,
    o2: f64,
    he: f64,
}

impl MixScratch {
    fn new(id: String) -> Self {
        MixScratch {
            id,
            o2: 0.0,
            he: 0.0,
        }
    }
}

/// Scratch for one `<tankdata>` block, holding raw Pascal pressures.
#[derive(Default)]
struct TankScratch {
    begin_pa: Option<f64>,
    end_pa: Option<f64>,
}

impl TankScratch {
    /// Convert to a `TankData` (Pa → bar), or `None` for an all-zero block.
    /// The Perdix pads with empty blocks; only populated ones are real.
    fn into_tank_data(self) -> Option<TankData> {
        let begin = self.begin_pa.unwrap_or(0.0);
        let end = self.end_pa.unwrap_or(0.0);
        if begin == 0.0 && end == 0.0 {
            return None;
        }
        Some(TankData {
            gas_index: None,
            volume: None,
            pressure_begin: Some(Bar(pa_to_bar(begin))),
            pressure_end: Some(Bar(pa_to_bar(end))),
        })
    }
}

#[derive(Default)]
struct WaypointScratch {
    offset_s: Option<i64>,
    depth_m: Option<f64>,
    temp_k: Option<f64>,
    ppo2: Option<f64>,
}

impl WaypointScratch {
    fn into_sample(self, gas_index: Option<u16>) -> Sample {
        Sample {
            offset: Seconds(self.offset_s.unwrap_or(0)),
            depth: Meters(self.depth_m.unwrap_or(0.0)),
            temperature: self.temp_k.map(|k| Celsius(kelvin_to_c(k))),
            // Waypoints in this dialect carry no per-sample tank pressure.
            tank_pressure: None,
            ppo2: self.ppo2,
            gas_index,
            // CCR setpoint not surfaced yet (divemode type is parsed for mode
            // detection only). TODO: derive setpoint from calculatedpo2/divemode.
            setpoint: None,
        }
    }
}

#[derive(Default)]
struct DeviceFields {
    model: Option<String>,
    name: Option<String>,
    serial: Option<String>,
}

impl DeviceFields {
    fn into_device_id(self) -> DeviceId {
        DeviceId {
            make: "Shearwater".to_string(),
            // Prefer <model>, fall back to <name> if a future export omits it.
            model: self
                .model
                .or(self.name)
                .unwrap_or_else(|| "Unknown".to_string()),
            serial: self.serial.unwrap_or_default(),
        }
    }
}

// --- text dispatch -----------------------------------------------------------

/// Route a captured text value to the right accumulator, based on the leaf
/// element name and the current region of the document.
#[allow(clippy::too_many_arguments)]
fn apply_text(
    target: &str,
    text: &str,
    in_divecomputer: bool,
    device: &mut DeviceFields,
    mix: Option<&mut MixScratch>,
    in_before: bool,
    in_after: bool,
    dive: Option<&mut DiveAcc>,
    wp: Option<&mut WaypointScratch>,
    tank: Option<&mut TankScratch>,
) {
    match target {
        "model" if in_divecomputer => device.model = Some(text.to_string()),
        "serialnumber" if in_divecomputer => device.serial = Some(text.to_string()),
        "name" if in_divecomputer && device.name.is_none() => device.name = Some(text.to_string()),
        "o2" => {
            if let Some(m) = mix {
                m.o2 = parse_f64(text);
            }
        }
        "he" => {
            if let Some(m) = mix {
                m.he = parse_f64(text);
            }
        }
        "divenumber" if in_before => {
            if let Some(d) = dive {
                d.dive_number = text.parse::<u32>().ok();
            }
        }
        "datetime" if in_before => {
            if let Some(d) = dive {
                d.start = parse_datetime(text);
            }
        }
        "diveduration" if in_after => {
            if let Some(d) = dive {
                d.duration_s = parse_f64_opt(text).map(|v| v as i64);
            }
        }
        "greatestdepth" if in_after => {
            if let Some(d) = dive {
                d.greatest_depth = parse_f64_opt(text);
            }
        }
        "averagedepth" if in_after => {
            if let Some(d) = dive {
                d.average_depth = parse_f64_opt(text);
            }
        }
        // Waypoint leaves.
        "depth" => {
            if let Some(w) = wp {
                w.depth_m = parse_f64_opt(text);
            }
        }
        "divetime" => {
            if let Some(w) = wp {
                w.offset_s = parse_f64_opt(text).map(|v| v as i64);
            }
        }
        "temperature" => {
            if let Some(w) = wp {
                w.temp_k = parse_f64_opt(text);
            }
        }
        "calculatedpo2" => {
            if let Some(w) = wp {
                w.ppo2 = parse_f64_opt(text);
            }
        }
        // Tank pressures are Pascals (scientific notation possible). Conversion
        // to bar happens when the block closes, so the zero-check is in Pascals.
        "tankpressurebegin" => {
            if let Some(t) = tank {
                t.begin_pa = parse_f64_opt(text);
            }
        }
        "tankpressureend" => {
            if let Some(t) = tank {
                t.end_pa = parse_f64_opt(text);
            }
        }
        _ => {}
    }
}

// --- small helpers -----------------------------------------------------------

/// Local element name: the part after the last `:` (drops any namespace prefix),
/// decoded lossily from the raw qualified-name bytes.
fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.into_owned(),
    }
}

/// Read an attribute value by local name from a start/empty tag.
fn attr_value(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == key {
            a.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

/// Resolve a `<switchmix ref>` to an index into the gas table.
fn gas_index_for(defs: &[GasDef], ref_id: &str) -> Option<u16> {
    defs.iter().position(|g| g.id == ref_id).map(|i| i as u16)
}

/// Parse an f64, defaulting to 0.0 (used where the dialect guarantees a value).
fn parse_f64(text: &str) -> f64 {
    text.parse::<f64>().unwrap_or(0.0)
}

/// Parse an f64 as an Option (handles scientific notation natively).
fn parse_f64_opt(text: &str) -> Option<f64> {
    text.parse::<f64>().ok()
}

/// Parse a UDDF datetime (RFC 3339, e.g. `2024-12-01T15:24:00Z`) to UTC.
fn parse_datetime(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
