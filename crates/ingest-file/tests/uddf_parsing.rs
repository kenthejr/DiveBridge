//! Integration tests for the Shearwater UDDF parser, run against real exports.

use divebridge_ingest_file::core::{SourceKind, TrackingKind};
use divebridge_ingest_file::{parse_uddf, to_dives};

const PERDIX2: &[u8] = include_bytes!("fixtures/perdix2-real.uddf");
const CCR: &[u8] = include_bytes!("fixtures/perdix-ccr-real.uddf");

#[test]
fn parses_perdix2_into_one_recording() {
    let sources = parse_uddf(PERDIX2).expect("perdix2 should parse");
    assert_eq!(sources.len(), 1, "one <dive> => one SourceRecording");

    let src = &sources[0];
    assert_eq!(src.kind, SourceKind::UddfFile);
    assert_eq!(src.device.make, "Shearwater");
    assert_eq!(src.device.serial, "A3B6F031");
    assert!(
        src.device.model.contains("Perdix 2"),
        "model was {:?}",
        src.device.model
    );
    assert_eq!(src.computer_dive_number, Some(42));
    assert_eq!(src.gases.len(), 2, "CC1 + OC1");

    assert_eq!(src.segments.len(), 1, "one segment for now");
    assert_eq!(
        src.segments[0].samples.len(),
        464,
        "464 waypoints => 464 samples"
    );
}

#[test]
fn tankdata_keeps_only_the_real_block() {
    // The Perdix emits ~6 <tankdata> blocks; all but the first are all-zero pads
    // and must be skipped. The real one carries Pascal pressures.
    let sources = parse_uddf(PERDIX2).expect("perdix2 should parse");
    let tanks = &sources[0].tanks;
    assert_eq!(tanks.len(), 1, "only the populated tankdata block is kept");

    let begin = tanks[0]
        .pressure_begin
        .expect("real tank has begin pressure");
    let end = tanks[0].pressure_end.expect("real tank has end pressure");
    // 1.949838E+07 Pa => ~194.98 bar; 6998181 Pa => ~69.98 bar.
    assert!(
        (begin.0 - 194.98).abs() < 0.1,
        "expected ~194.98 bar begin, got {}",
        begin.0
    );
    assert!(
        (end.0 - 69.98).abs() < 0.1,
        "expected ~69.98 bar end, got {}",
        end.0
    );
}

#[test]
fn first_waypoint_temperature_kelvin_to_celsius() {
    let sources = parse_uddf(PERDIX2).expect("perdix2 should parse");
    let first = sources[0].segments[0].samples[0];
    let temp = first.temperature.expect("first waypoint has a temperature");
    // 281.15 K - 273.15 = 8.00 C
    assert!(
        (temp.0 - 8.0).abs() < 0.01,
        "expected ~8.0 C, got {}",
        temp.0
    );
}

#[test]
fn max_depth_from_greatestdepth() {
    let sources = parse_uddf(PERDIX2).expect("perdix2 should parse");
    let max = sources[0].segments[0].max_depth.0;
    assert!((max - 32.9).abs() < 0.1, "expected ~32.9 m, got {}", max);
}

#[test]
fn active_gas_carried_forward() {
    // Only one <switchmix> is emitted (at waypoint 0). Every later waypoint must
    // inherit that gas index.
    let sources = parse_uddf(PERDIX2).expect("perdix2 should parse");
    let samples = &sources[0].segments[0].samples;
    assert_eq!(samples[0].gas_index, Some(0), "CC1:21/00 is index 0");
    assert_eq!(
        samples[200].gas_index,
        Some(0),
        "gas index carried forward to later waypoints"
    );
}

#[test]
fn to_dives_builds_one_tracked_dive() {
    let sources = parse_uddf(PERDIX2).expect("perdix2 should parse");
    let dives = to_dives(sources);
    assert_eq!(dives.len(), 1);

    let dive = &dives[0];
    assert_eq!(dive.tracking, TrackingKind::Tracked);
    assert_eq!(dive.summary.descent_count, 1);
    assert!(
        (dive.summary.max_depth.0 - 32.9).abs() < 0.1,
        "summary max_depth was {}",
        dive.summary.max_depth.0
    );
    // dedup_key => serial::dive_number.
    assert_eq!(dive.id.0, "A3B6F031::42");
    // primary_source resolves back to the lone source.
    assert!(dive.primary().is_some());

    // Primary tank's begin pressure is surfaced on the summary for SSI mapping.
    let pressure_start = dive
        .summary
        .pressure_start
        .expect("summary carries the primary tank begin pressure");
    assert!(
        (pressure_start.0 - 194.98).abs() < 0.1,
        "summary pressure_start was {}",
        pressure_start.0
    );
}

#[test]
fn parses_ccr_export_with_ten_samples() {
    let sources = parse_uddf(CCR).expect("ccr should parse");
    assert_eq!(sources.len(), 1);

    let src = &sources[0];
    assert_eq!(src.segments[0].samples.len(), 10, "10 waypoints");
    assert_eq!(src.computer_dive_number, Some(963));
    assert_eq!(src.gases.len(), 1, "single CC4 mix");

    // CCR detection proxy: ppO2 is present (calculatedpo2) on the first sample,
    // and the gas resolves to the closed-circuit mix.
    assert_eq!(src.segments[0].samples[0].gas_index, Some(0));
    assert!(src.segments[0].samples[0].ppo2.is_some());
}
