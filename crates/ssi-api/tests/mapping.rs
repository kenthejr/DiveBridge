//! Offline tests for the pure SSI mapping + form encoding. No network calls.

use std::collections::HashSet;

use chrono::TimeZone;
use divebridge_ssi_api as ssi;
use ssi::core::{
    Bar, Celsius, DeviceId, Dive, DiveId, DiveLog, DiveSummary, GasMix, Kilograms, Meters, Seconds,
    SourceId, SourceKind, SourceRecording, SyncState, TrackingKind, Weather,
};
use ssi::{build_create_form, encode_form, SsiError, SubmitContext, SSI_CREATE_SOURCE};

/// Construct a Tracked dive mirroring `core`'s `sample_dive`, with the values the
/// fixture exercises (236/126 bar, 13.7m depth, 14.4C water, EAN36, etc.).
fn fixture_like_dive(tracking: TrackingKind) -> Dive {
    let start = chrono::Utc.with_ymd_and_hms(2026, 6, 26, 8, 30, 0).unwrap();
    let source = SourceRecording {
        id: SourceId("src-1".into()),
        device: DeviceId {
            make: "Shearwater".into(),
            model: "Perdix 2".into(),
            serial: "SN123".into(),
        },
        kind: SourceKind::ShearwaterBle,
        imported_at: start,
        computer_dive_number: Some(42),
        original_artifact: None,
        gases: vec![GasMix {
            o2_percent: 36.0,
            he_percent: 0.0,
        }],
        tanks: vec![],
        segments: vec![],
        gps_track: vec![],
    };
    Dive {
        id: DiveId("dive-1".into()),
        tracking,
        primary_source: SourceId("src-1".into()),
        sources: vec![source],
        summary: DiveSummary {
            start,
            total_runtime: Seconds(2520),
            total_bottom_time: Seconds(2520), // 42 min
            max_depth: Meters(13.7),
            avg_depth: Some(Meters(7.3)),
            descent_count: 1,
            min_temp: Some(Celsius(14.4)),
            gases: vec![GasMix {
                o2_percent: 36.0,
                he_percent: 0.0,
            }],
            pressure_start: Some(Bar(236.0)),
            pressure_end: Some(Bar(126.0)),
        },
        log: DiveLog {
            weight: Some(Kilograms(8.2)),
            visibility: Some(Meters(3.7)),
            notes: Some("nice viz".into()),
            weather: Some(Weather {
                air_temp_c: Some(30.0),
                ..Default::default()
            }),
            ..Default::default()
        },
        sync: SyncState::default(),
        verification: None,
    }
}

fn ctx() -> SubmitContext {
    SubmitContext {
        user_master_id: "999".into(),
        dive_nr: 45,
        dive_sites_id: Some("187286".into()),
        dive_site_bow: Some("fresh".into()),
        var_tanktype_id: Some("19".into()),
        gearconfiguration_id: Some("66".into()),
        specialdive_ids: vec!["27".into(), "43".into()],
        buddy_ids: vec!["3489556".into(), "111".into()],
        ..Default::default()
    }
}

fn lookup<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

#[test]
fn maps_units_date_gas_and_constants() {
    let dive = fixture_like_dive(TrackingKind::Tracked);
    let fields = build_create_form(&dive, &ctx()).expect("tracked dive should map");

    assert_eq!(lookup(&fields, "odin_user_log_depth_m"), Some("13.7"));
    assert_eq!(lookup(&fields, "odin_user_log_depth_ft"), Some("45"));
    assert_eq!(lookup(&fields, "odin_user_log_avg_depth_m"), Some("7.3"));
    assert_eq!(lookup(&fields, "odin_user_log_avg_depth_ft"), Some("24"));

    assert_eq!(
        lookup(&fields, "odin_user_log_pressure_start_bar"),
        Some("236")
    );
    assert_eq!(
        lookup(&fields, "odin_user_log_pressure_end_bar"),
        Some("126")
    );

    // psi close to fixture (3421 / 1824). We use the spec-mandated factor
    // 14.5037738, which yields 3423/1828; SSI's own server rounds slightly
    // differently in the capture, so allow a small tolerance.
    let psi_start: i64 = lookup(&fields, "odin_user_log_pressure_start_psi")
        .unwrap()
        .parse()
        .unwrap();
    assert!((psi_start - 3421).abs() <= 3, "psi_start={psi_start}");
    let psi_end: i64 = lookup(&fields, "odin_user_log_pressure_end_psi")
        .unwrap()
        .parse()
        .unwrap();
    assert!((psi_end - 1824).abs() <= 5, "psi_end={psi_end}");

    // weight 8.2kg -> 18lb.
    assert_eq!(lookup(&fields, "odin_user_log_weight_kg"), Some("8.2"));
    assert_eq!(lookup(&fields, "odin_user_log_weight_lb"), Some("18"));

    // watertemp 14.4C -> 58F.
    assert_eq!(lookup(&fields, "odin_user_log_watertemp_c"), Some("14.4"));
    assert_eq!(lookup(&fields, "odin_user_log_watertemp_f"), Some("58"));
    // airtemp 30C -> 86F.
    assert_eq!(lookup(&fields, "odin_user_log_airtemp_c"), Some("30"));
    assert_eq!(lookup(&fields, "odin_user_log_airtemp_f"), Some("86"));

    // vis 3.7m -> 12ft.
    assert_eq!(lookup(&fields, "odin_user_log_vis_m"), Some("3.7"));
    assert_eq!(lookup(&fields, "odin_user_log_vis_ft"), Some("12"));

    // Gas: EAN36.
    assert_eq!(lookup(&fields, "odin_user_log_ean"), Some("1"));
    assert_eq!(lookup(&fields, "odin_user_log_ean_percent"), Some("36"));

    // Date split + entry time + divetime.
    assert_eq!(lookup(&fields, "date_sel2_dd"), Some("26"));
    assert_eq!(lookup(&fields, "date_sel2_mm"), Some("06"));
    assert_eq!(lookup(&fields, "date_sel2_yy"), Some("2026"));
    assert_eq!(lookup(&fields, "odin_user_log_entry_time"), Some("08:30"));
    assert_eq!(lookup(&fields, "odin_user_log_divetime"), Some("42"));

    // Constants + safety.
    assert_eq!(lookup(&fields, "source"), Some(SSI_CREATE_SOURCE));
    assert_eq!(lookup(&fields, "submit"), Some("Submit"));
    assert_eq!(lookup(&fields, "log_linked_facility_id"), Some(""));

    // Context defaults.
    assert_eq!(lookup(&fields, "odin_user_log_dive_nr"), Some("45"));
    assert_eq!(lookup(&fields, "odin_user_log_user_master_id"), Some("999"));
    assert_eq!(
        lookup(&fields, "odin_user_log_dive_sites_id"),
        Some("187286")
    );
    assert_eq!(lookup(&fields, "dive_site_bow"), Some("fresh"));
    // Default Fun Dive when divetype not supplied.
    assert_eq!(lookup(&fields, "odin_user_log_var_divetype_id"), Some("24"));

    // Non-simple imperial fields left blank.
    assert_eq!(lookup(&fields, "odin_user_log_tank_vol_cuft"), Some(""));
    assert_eq!(lookup(&fields, "odin_user_log_amv_psi"), Some(""));
}

#[test]
fn air_dive_sets_ean_zero() {
    let mut dive = fixture_like_dive(TrackingKind::Tracked);
    dive.sources[0].gases = vec![GasMix::AIR];
    let fields = build_create_form(&dive, &ctx()).unwrap();
    assert_eq!(lookup(&fields, "odin_user_log_ean"), Some("0"));
    assert_eq!(lookup(&fields, "odin_user_log_ean_percent"), Some(""));
}

#[test]
fn training_dive_is_rejected() {
    let dive = fixture_like_dive(TrackingKind::Training);
    let err = build_create_form(&dive, &ctx()).unwrap_err();
    assert!(matches!(err, SsiError::NotUploadable));
}

#[test]
fn every_emitted_field_is_a_known_fixture_field() {
    // Parse the canonical field names from the sanitized fixture.
    let fixture = include_str!("fixtures/create-dive.request.txt");
    let known: HashSet<String> = fixture
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split('=').next())
        .map(|k| {
            k.trim_end_matches("%5B%5D")
                .trim_end_matches("[]")
                .to_string()
        })
        .collect();

    assert!(known.contains("odin_user_log_user_master_id"));

    let dive = fixture_like_dive(TrackingKind::Tracked);
    let fields = build_create_form(&dive, &ctx()).unwrap();
    for (k, _) in &fields {
        let base = k.trim_end_matches("[]");
        assert!(
            known.contains(base),
            "emitted field {k:?} (base {base:?}) is not in the known fixture field set"
        );
    }
}

#[test]
fn encode_emits_repeated_keys() {
    let fields = vec![
        ("odin_user_log_buddy_ids[]".to_string(), "111".to_string()),
        ("odin_user_log_buddy_ids[]".to_string(), "222".to_string()),
    ];
    let body = encode_form(&fields);
    // The key (percent-encoded `[]` -> %5B%5D) appears twice.
    let needle = "odin_user_log_buddy_ids%5B%5D=";
    let count = body.matches(needle).count();
    assert_eq!(count, 2, "body={body}");
    assert!(body.contains("=111"));
    assert!(body.contains("=222"));
}
