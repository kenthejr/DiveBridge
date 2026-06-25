//! Deterministic classification rules (no AI).
//!
//! Rules assign a *default* [`TrackingKind`] and/or tags to a dive based on
//! signals available at import time. A manual override on the dive always wins
//! over rules — `classify` only computes the default.
//!
//! MVP signals: max depth (pool detector), assigned site name, and GPS proximity
//! (once a GPS source is present). Date/time windows can be added later.

use serde::{Deserialize, Serialize};

use crate::model::{DiveSite, DiveSummary};
use crate::tracking::TrackingKind;

/// A predicate over a dive's summary + (optional) assigned site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuleMatcher {
    /// Max depth strictly below this many meters (e.g. pool < 3 m).
    MaxDepthBelowMeters(f64),
    /// Assigned site name equals this (case-insensitive).
    SiteNameEquals(String),
    /// Site GPS within `radius_m` of (`lat`, `lon`).
    GpsWithin { lat: f64, lon: f64, radius_m: f64 },
    /// All sub-matchers must match.
    All(Vec<RuleMatcher>),
    /// Any sub-matcher matches.
    Any(Vec<RuleMatcher>),
}

/// Inputs available to rule evaluation.
pub struct RuleContext<'a> {
    pub summary: &'a DiveSummary,
    pub site: Option<&'a DiveSite>,
}

impl RuleMatcher {
    pub fn matches(&self, ctx: &RuleContext) -> bool {
        match self {
            RuleMatcher::MaxDepthBelowMeters(m) => ctx.summary.max_depth.0 < *m,
            RuleMatcher::SiteNameEquals(name) => ctx
                .site
                .map(|s| s.name.eq_ignore_ascii_case(name))
                .unwrap_or(false),
            RuleMatcher::GpsWithin { lat, lon, radius_m } => ctx
                .site
                .and_then(|s| s.gps)
                .map(|g| haversine_m(g.lat, g.lon, *lat, *lon) <= *radius_m)
                .unwrap_or(false),
            RuleMatcher::All(ms) => ms.iter().all(|m| m.matches(ctx)),
            RuleMatcher::Any(ms) => ms.iter().any(|m| m.matches(ctx)),
        }
    }
}

/// A user-configured classification rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationRule {
    pub name: String,
    pub matcher: RuleMatcher,
    pub assign_tracking: Option<TrackingKind>,
    #[serde(default)]
    pub add_tags: Vec<String>,
}

/// Result of applying the rule set: a suggested default classification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Classification {
    /// First matching rule with an `assign_tracking` wins.
    pub tracking: Option<TrackingKind>,
    /// Union of tags from all matching rules.
    pub tags: Vec<String>,
}

/// Evaluate rules in order. Tracking is decided by the first matching rule that
/// assigns one; tags accumulate across all matches (deduped, order-preserving).
pub fn classify(rules: &[ClassificationRule], ctx: &RuleContext) -> Classification {
    let mut out = Classification::default();
    for rule in rules {
        if !rule.matcher.matches(ctx) {
            continue;
        }
        if out.tracking.is_none() {
            if let Some(t) = rule.assign_tracking {
                out.tracking = Some(t);
            }
        }
        for tag in &rule.add_tags {
            if !out.tags.iter().any(|t| t == tag) {
                out.tags.push(tag.clone());
            }
        }
    }
    out
}

/// Great-circle distance in meters between two lat/lon points.
fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0; // mean Earth radius (m)
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}
