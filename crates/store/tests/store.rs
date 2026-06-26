//! Integration tests for `divebridge-store`.

use std::collections::BTreeMap;

use chrono::TimeZone;
use divebridge_core::model::{
    DeviceId, Dive, DiveId, DiveLog, DiveSummary, GasMix, Segment, SourceId, SourceKind,
    SourceRecording, SyncRecord, SyncState, SyncStatus,
};
use divebridge_core::tracking::TrackingKind;
use divebridge_core::units::{Celsius, Meters, Seconds};
use divebridge_core::ClassificationRule;
use divebridge_core::RuleMatcher;

use divebridge_store::{Store, StoreError, UpsertOutcome};
use tempfile::tempdir;

/// Build a minimal dive (shape mirrors core's `sample_dive`).
fn sample_dive(dive_id: &str, dive_number: u32) -> Dive {
    let start = chrono::Utc.with_ymd_and_hms(2026, 6, 1, 14, 0, 0).unwrap();
    let source = sample_source("src-1", "SN123", dive_number, start);
    Dive {
        id: DiveId(dive_id.into()),
        tracking: TrackingKind::default(),
        primary_source: SourceId("src-1".into()),
        sources: vec![source],
        summary: DiveSummary {
            start,
            total_runtime: Seconds(1800),
            total_bottom_time: Seconds(1800),
            max_depth: Meters(30.0),
            avg_depth: Some(Meters(15.0)),
            descent_count: 1,
            min_temp: Some(Celsius(18.0)),
            gases: vec![GasMix::AIR],
            pressure_start: None,
            pressure_end: None,
        },
        log: DiveLog::default(),
        sync: SyncState::default(),
        verification: None,
    }
}

fn sample_source(
    id: &str,
    serial: &str,
    dive_number: u32,
    start: chrono::DateTime<chrono::Utc>,
) -> SourceRecording {
    SourceRecording {
        id: SourceId(id.into()),
        device: DeviceId {
            make: "Shearwater".into(),
            model: "Perdix 2".into(),
            serial: serial.into(),
        },
        kind: SourceKind::ShearwaterBle,
        imported_at: start,
        computer_dive_number: Some(dive_number),
        original_artifact: None,
        gases: vec![GasMix::AIR],
        tanks: vec![],
        segments: vec![Segment {
            start,
            duration: Seconds(1800),
            max_depth: Meters(30.0),
            avg_depth: Some(Meters(15.0)),
            min_temp: Some(Celsius(18.0)),
            samples: vec![],
        }],
        gps_track: vec![],
    }
}

#[test]
fn open_creates_directories() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().join("data");
    let _store = Store::open(&root).unwrap();
    assert!(root.is_dir());
    assert!(root.join("dives").is_dir());
    assert!(root.join("raw").is_dir());
}

#[test]
fn save_dive_then_load_dive_round_trips() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    let dive = sample_dive("dive-1", 42);
    store.save_dive(&dive).unwrap();

    let loaded = store.load_dive(&DiveId("dive-1".into())).unwrap();
    assert_eq!(loaded, dive);
}

#[test]
fn load_missing_dive_is_not_found() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    let err = store.load_dive(&DiveId("nope".into())).unwrap_err();
    assert!(matches!(err, StoreError::NotFound(_)));
}

#[test]
fn list_dive_ids_returns_saved_ids() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    assert!(store.list_dive_ids().unwrap().is_empty());

    store.save_dive(&sample_dive("dive-1", 1)).unwrap();
    store.save_dive(&sample_dive("dive-2", 2)).unwrap();

    let mut ids: Vec<String> = store
        .list_dive_ids()
        .unwrap()
        .into_iter()
        .map(|d| d.0)
        .collect();
    ids.sort();
    assert_eq!(ids, vec!["dive-1".to_string(), "dive-2".to_string()]);
}

#[test]
fn delete_dive_removes_file_and_ledger_entry() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    store.save_dive(&sample_dive("dive-1", 1)).unwrap();
    store.delete_dive(&DiveId("dive-1".into())).unwrap();

    assert!(store.list_dive_ids().unwrap().is_empty());
    let ledger: BTreeMap<String, SyncState> =
        serde_json::from_slice(&std::fs::read(tmp.path().join("ledger.json")).unwrap()).unwrap();
    assert!(!ledger.contains_key("dive-1"));
}

#[test]
fn upsert_source_is_idempotent() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    let dive_id = DiveId("dive-1".into());
    store.save_dive(&sample_dive("dive-1", 42)).unwrap();

    let start = chrono::Utc.with_ymd_and_hms(2026, 6, 1, 14, 0, 0).unwrap();

    // Same dedup_key (serial SN123, dive_number 42) => Unchanged.
    let same = sample_source("src-dup", "SN123", 42, start);
    assert_eq!(
        store.upsert_source(&dive_id, same).unwrap(),
        UpsertOutcome::Unchanged
    );
    assert_eq!(store.load_dive(&dive_id).unwrap().sources.len(), 1);

    // Different dive_number => Added.
    let different = sample_source("src-2", "SN123", 43, start);
    assert_eq!(
        store.upsert_source(&dive_id, different).unwrap(),
        UpsertOutcome::Added
    );
    assert_eq!(store.load_dive(&dive_id).unwrap().sources.len(), 2);
}

#[test]
fn save_raw_artifact_writes_bytes_and_returns_sha256() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    let source_id = SourceId("src-1".into());
    let bytes = b"<uddf>verbatim</uddf>";
    let artifact = store
        .save_raw_artifact(&source_id, "export.uddf", bytes)
        .unwrap();

    assert_eq!(artifact.path, "raw/src-1/export.uddf");
    assert_eq!(artifact.bytes, bytes.len() as u64);
    assert_eq!(artifact.sha256, divebridge_core::hash::sha256_hex(bytes));

    let on_disk = std::fs::read(tmp.path().join("raw/src-1/export.uddf")).unwrap();
    assert_eq!(on_disk, bytes);
}

#[test]
fn save_raw_artifact_identical_bytes_is_ok() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    let source_id = SourceId("src-1".into());
    let bytes = b"same bytes";

    let first = store.save_raw_artifact(&source_id, "f.bin", bytes).unwrap();
    let second = store.save_raw_artifact(&source_id, "f.bin", bytes).unwrap();
    assert_eq!(first, second);
}

#[test]
fn save_raw_artifact_different_bytes_is_immutable_error() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();
    let source_id = SourceId("src-1".into());

    store
        .save_raw_artifact(&source_id, "f.bin", b"original")
        .unwrap();
    let err = store
        .save_raw_artifact(&source_id, "f.bin", b"tampered")
        .unwrap_err();
    assert!(matches!(err, StoreError::Immutable(_)));
}

#[test]
fn load_rules_empty_then_round_trips() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    assert!(store.load_rules().unwrap().is_empty());

    let rules = vec![ClassificationRule {
        name: "Home pool".into(),
        matcher: RuleMatcher::MaxDepthBelowMeters(3.0),
        assign_tracking: Some(TrackingKind::Training),
        add_tags: vec!["pool".into()],
    }];
    store.save_rules(&rules).unwrap();
    assert_eq!(store.load_rules().unwrap(), rules);
}

#[test]
fn merges_empty_then_round_trips() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    assert!(store.load_merges().unwrap().is_empty());

    store
        .record_merge(
            &DiveId("dive-1".into()),
            &[SourceId("src-1".into()), SourceId("src-2".into())],
        )
        .unwrap();

    let merges = store.load_merges().unwrap();
    assert_eq!(
        merges.get("dive-1"),
        Some(&vec!["src-1".to_string(), "src-2".to_string()])
    );
}

#[test]
fn save_dive_writes_ledger_with_sync_state() {
    let tmp = tempdir().unwrap();
    let store = Store::open(tmp.path()).unwrap();

    let mut dive = sample_dive("dive-1", 1);
    dive.sync = SyncState {
        records: vec![SyncRecord {
            target: "ssi".into(),
            remote_id: Some("remote-42".into()),
            synced_content_hash: Some("abc".into()),
            last_synced: None,
            status: SyncStatus::Synced,
        }],
    };
    store.save_dive(&dive).unwrap();

    let ledger: BTreeMap<String, SyncState> =
        serde_json::from_slice(&std::fs::read(tmp.path().join("ledger.json")).unwrap()).unwrap();
    assert_eq!(ledger.get("dive-1"), Some(&dive.sync));
}
