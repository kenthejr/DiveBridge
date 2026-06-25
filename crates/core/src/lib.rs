//! DiveBridge core domain model — see `CONTRACT.md`.
//!
//! This crate is intentionally dependency-light and IO-free so it can be the
//! frozen contract every other crate (and parallel agent) builds against.

pub mod hash;
pub mod model;
pub mod rules;
pub mod tracking;
pub mod units;

pub use model::*;
pub use rules::{classify, Classification, ClassificationRule, RuleContext, RuleMatcher};
pub use tracking::TrackingKind;
pub use units::{Bar, Celsius, Kilograms, Liters, Meters, Seconds};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_dive(max_depth_m: f64, site: Option<DiveSite>) -> Dive {
        let start = chrono::Utc.with_ymd_and_hms(2026, 6, 1, 14, 0, 0).unwrap();
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
            gases: vec![GasMix::AIR],
            segments: vec![Segment {
                start,
                duration: Seconds(1800),
                max_depth: Meters(max_depth_m),
                avg_depth: Some(Meters(max_depth_m / 2.0)),
                min_temp: Some(Celsius(18.0)),
                samples: vec![],
            }],
            gps_track: vec![],
        };
        Dive {
            id: DiveId("dive-1".into()),
            tracking: TrackingKind::default(),
            primary_source: SourceId("src-1".into()),
            sources: vec![source],
            summary: DiveSummary {
                start,
                total_runtime: Seconds(1800),
                total_bottom_time: Seconds(1800),
                max_depth: Meters(max_depth_m),
                avg_depth: Some(Meters(max_depth_m / 2.0)),
                descent_count: 1,
                min_temp: Some(Celsius(18.0)),
                gases: vec![GasMix::AIR],
            },
            log: DiveLog {
                site,
                ..Default::default()
            },
            sync: SyncState::default(),
            verification: None,
        }
    }

    #[test]
    fn new_dives_default_to_tracked_and_uploadable() {
        let d = sample_dive(30.0, None);
        assert_eq!(d.tracking, TrackingKind::Tracked);
        assert!(d.is_uploadable());
    }

    #[test]
    fn dedup_key_prefers_serial_and_dive_number() {
        let d = sample_dive(30.0, None);
        assert_eq!(d.primary().unwrap().dedup_key(), "SN123::42");
    }

    #[test]
    fn content_hash_is_stable_and_changes_with_edits() {
        let mut d = sample_dive(30.0, None);
        let h1 = d.content_hash();
        assert_eq!(h1, d.content_hash());
        d.log.notes = Some("great viz".into());
        assert_ne!(h1, d.content_hash());
    }

    #[test]
    fn pool_rule_classifies_shallow_dive_as_training() {
        let rules = vec![ClassificationRule {
            name: "Home pool".into(),
            matcher: RuleMatcher::MaxDepthBelowMeters(3.0),
            assign_tracking: Some(TrackingKind::Training),
            add_tags: vec!["pool".into()],
        }];
        let pool = sample_dive(2.0, None);
        let ctx = RuleContext {
            summary: &pool.summary,
            site: pool.log.site.as_ref(),
        };
        let c = classify(&rules, &ctx);
        assert_eq!(c.tracking, Some(TrackingKind::Training));
        assert_eq!(c.tags, vec!["pool".to_string()]);

        let deep = sample_dive(30.0, None);
        let ctx2 = RuleContext {
            summary: &deep.summary,
            site: None,
        };
        assert_eq!(classify(&rules, &ctx2).tracking, None);
    }

    #[test]
    fn gps_rule_matches_within_radius() {
        let rules = vec![ClassificationRule {
            name: "Folsom Lake".into(),
            matcher: RuleMatcher::GpsWithin {
                lat: 38.71,
                lon: -121.14,
                radius_m: 2000.0,
            },
            assign_tracking: Some(TrackingKind::Tracked),
            add_tags: vec!["folsom".into()],
        }];
        let site = DiveSite {
            name: "Folsom".into(),
            gps: Some(GpsPoint {
                lat: 38.715,
                lon: -121.145,
            }),
            ssi_site_id: None,
        };
        let d = sample_dive(12.0, Some(site));
        let ctx = RuleContext {
            summary: &d.summary,
            site: d.log.site.as_ref(),
        };
        assert_eq!(classify(&rules, &ctx).tracking, Some(TrackingKind::Tracked));
    }
}
