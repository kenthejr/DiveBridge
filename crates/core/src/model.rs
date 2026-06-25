//! The DiveBridge domain model.
//!
//! Design (from spec grilling):
//! - A logical [`Dive`] owns one or more [`SourceRecording`]s. This single
//!   `sources` list covers BOTH axes of multiplicity:
//!     * time-merge — several separate device recordings (e.g. spearfishing
//!       surface intervals) merged into one logical dive, and
//!     * cross-device — the same submersion recorded by multiple computers
//!       (e.g. Perdix 2 + Garmin Mk3i).
//! - One source is the `primary_source` used for the canonical profile/graphs.
//! - Raw layer (`SourceRecording`, including the verbatim `original_artifact`)
//!   is IMMUTABLE and verifiable for insurance. The editable [`DiveLog`] overlay
//!   is what we prepare for SSI.
//! - Each `SourceRecording` may contain multiple [`Segment`]s (submersions)
//!   separated by surface intervals.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::hash::sha256_hex;
use crate::tracking::TrackingKind;
use crate::units::{Bar, Celsius, Kilograms, Liters, Meters, Seconds};

/// Stable identity of a logical dive (persisted; survives merges & re-syncs).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiveId(pub String);

/// Identity of one source recording.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

/// Which device produced a recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceId {
    pub make: String,
    pub model: String,
    pub serial: String,
}

/// How a recording entered DiveBridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    /// Pulled directly from a Shearwater computer over BLE.
    ShearwaterBle,
    /// Imported from a UDDF/XML export file.
    UddfFile,
    /// Imported from a Garmin FIT file (deferred; seam only).
    GarminFit,
    /// Anything else (CSV, manual, …).
    Other,
}

/// Pointer to a verbatim original export, kept for verifiability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// Path within the local store.
    pub path: String,
    /// SHA-256 of the original bytes.
    pub sha256: String,
    pub bytes: u64,
}

/// A single GPS fix (from an external source — the Perdix 2 has no GPS).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpsPoint {
    pub lat: f64,
    pub lon: f64,
}

/// A breathing gas mixture, as percentages. Air == 21/0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GasMix {
    pub o2_percent: f64,
    pub he_percent: f64,
}

impl GasMix {
    pub const AIR: GasMix = GasMix {
        o2_percent: 21.0,
        he_percent: 0.0,
    };

    pub fn n2_percent(&self) -> f64 {
        (100.0 - self.o2_percent - self.he_percent).max(0.0)
    }

    pub fn is_air(&self) -> bool {
        (self.o2_percent - 21.0).abs() < 0.5 && self.he_percent < 0.5
    }
}

impl Default for GasMix {
    fn default() -> Self {
        GasMix::AIR
    }
}

/// Tank pressure data for one cylinder over a dive (begin/end + optional meta).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct TankData {
    pub gas_index: Option<u16>,
    pub volume: Option<Liters>,
    pub pressure_begin: Option<Bar>,
    pub pressure_end: Option<Bar>,
}

/// One profile data point within a [`Segment`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    /// Offset from the segment start.
    pub offset: Seconds,
    pub depth: Meters,
    pub temperature: Option<Celsius>,
    pub tank_pressure: Option<Bar>,
    /// Measured/computed ppO2 (bar). Useful for CCR.
    pub ppo2: Option<f64>,
    /// Index into the owning [`SourceRecording::gases`] active at this sample.
    pub gas_index: Option<u16>,
    /// CCR setpoint (bar), if applicable.
    pub setpoint: Option<f64>,
}

/// One continuous submersion. A recording with surface intervals (e.g.
/// repetitive freediving) is auto-split into multiple segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: DateTime<Utc>,
    pub duration: Seconds,
    pub max_depth: Meters,
    pub avg_depth: Option<Meters>,
    pub min_temp: Option<Celsius>,
    pub samples: Vec<Sample>,
}

/// An immutable raw recording from one device. The unit of provenance & dedup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRecording {
    pub id: SourceId,
    pub device: DeviceId,
    pub kind: SourceKind,
    pub imported_at: DateTime<Utc>,
    /// The computer's own dive counter, when exposed (stable dedup key).
    pub computer_dive_number: Option<u32>,
    /// Verbatim original export, for verifiability.
    pub original_artifact: Option<ArtifactRef>,
    /// Gas mixes referenced by `Sample::gas_index`.
    pub gases: Vec<GasMix>,
    /// Per-cylinder tank pressure data (begin/end), when the source reports it.
    pub tanks: Vec<TankData>,
    pub segments: Vec<Segment>,
    /// Optional GPS track from a GPS-capable source.
    pub gps_track: Vec<GpsPoint>,
}

impl SourceRecording {
    /// Stable dedup key: `serial::dive_number` when available, else a content
    /// hash of the first segment's identifying fields.
    pub fn dedup_key(&self) -> String {
        if let Some(n) = self.computer_dive_number {
            format!("{}::{}", self.device.serial, n)
        } else if let Some(seg) = self.segments.first() {
            let basis = format!(
                "{}|{}|{}|{}",
                self.device.serial,
                seg.start.to_rfc3339(),
                seg.max_depth.0,
                seg.duration.0
            );
            sha256_hex(basis.as_bytes())
        } else {
            sha256_hex(self.id.0.as_bytes())
        }
    }
}

/// A dive site. `ssi_site_id` is resolved against SSI's site DB at upload time
/// and cached.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DiveSite {
    pub name: String,
    pub gps: Option<GpsPoint>,
    pub ssi_site_id: Option<String>,
}

/// Weather enrichment (from data feeds; deterministic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Weather {
    pub air_temp_c: Option<f64>,
    pub wind_kph: Option<f64>,
    pub conditions: Option<String>,
}

/// Ocean/marine enrichment (from data feeds; deterministic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Ocean {
    pub sst_c: Option<f64>,
    pub tide_phase: Option<String>,
    pub current_kph: Option<f64>,
}

/// User overrides for summary fields when preparing the SSI submission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SummaryOverrides {
    pub max_depth: Option<Meters>,
    pub bottom_time: Option<Seconds>,
    pub visibility: Option<Meters>,
}

/// Editable overlay prepared for SSI. Never mutates the raw layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DiveLog {
    pub site: Option<DiveSite>,
    pub buddies: Vec<String>,
    pub tags: Vec<String>,
    pub dive_type: Option<String>,
    pub entry_type: Option<String>,
    pub weight: Option<Kilograms>,
    pub visibility: Option<Meters>,
    pub notes: Option<String>,
    pub weather: Option<Weather>,
    pub ocean: Option<Ocean>,
    pub overrides: SummaryOverrides,
}

/// Derived aggregate across the dive's (primary/merged) segments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiveSummary {
    pub start: DateTime<Utc>,
    /// Wall-clock from first descent to last ascent (includes surface intervals).
    pub total_runtime: Seconds,
    /// Time actually submerged (sum of segment durations).
    pub total_bottom_time: Seconds,
    pub max_depth: Meters,
    pub avg_depth: Option<Meters>,
    /// Number of submersions (segments) — good for habit analysis.
    pub descent_count: u32,
    pub min_temp: Option<Celsius>,
    pub gases: Vec<GasMix>,
    /// Primary tank's begin pressure (convenience for SSI mapping).
    pub pressure_start: Option<Bar>,
    /// Primary tank's end pressure (convenience for SSI mapping).
    pub pressure_end: Option<Bar>,
}

/// Sync status against one upload target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SyncStatus {
    #[default]
    NotSynced,
    Synced,
    /// Uploaded, but the local content has since changed.
    Stale,
    Failed(String),
}

/// Per-target sync ledger entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncRecord {
    /// e.g. "ssi".
    pub target: String,
    pub remote_id: Option<String>,
    /// `Dive::content_hash()` captured at last successful sync.
    pub synced_content_hash: Option<String>,
    pub last_synced: Option<DateTime<Utc>>,
    pub status: SyncStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SyncState {
    pub records: Vec<SyncRecord>,
}

impl SyncState {
    pub fn for_target(&self, target: &str) -> Option<&SyncRecord> {
        self.records.iter().find(|r| r.target == target)
    }
}

/// Tamper-evidence over the immutable raw layer (insurance verifiability).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawVerification {
    /// SHA-256 over the canonical manifest of raw sources + artifacts.
    pub manifest_sha256: String,
    /// Detached SSH signature (id_ed25519) over the manifest, if signed.
    pub signature: Option<String>,
    pub signed_at: Option<DateTime<Utc>>,
}

/// A logical dive: the top-level aggregate persisted by the store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dive {
    pub id: DiveId,
    pub tracking: TrackingKind,
    /// Which of `sources` provides the canonical profile/graphs.
    pub primary_source: SourceId,
    pub sources: Vec<SourceRecording>,
    pub summary: DiveSummary,
    pub log: DiveLog,
    pub sync: SyncState,
    pub verification: Option<RawVerification>,
}

impl Dive {
    pub fn primary(&self) -> Option<&SourceRecording> {
        self.sources.iter().find(|s| s.id == self.primary_source)
    }

    /// Hash of the upload-relevant content (summary + editable log + tracking).
    /// Used to detect whether a previously-synced dive has changed.
    pub fn content_hash(&self) -> String {
        let canonical = serde_json::json!({
            "tracking": self.tracking,
            "summary": self.summary,
            "log": self.log,
        });
        let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
        sha256_hex(&bytes)
    }

    /// Whether this dive is eligible for SSI upload (only `Tracked` dives are).
    pub fn is_uploadable(&self) -> bool {
        matches!(self.tracking, TrackingKind::Tracked)
    }
}
