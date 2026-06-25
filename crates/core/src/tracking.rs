//! Training vs. tracked classification of a dive.

use serde::{Deserialize, Serialize};

/// How a dive is treated by DiveBridge.
///
/// Mutually exclusive. New dives default to [`TrackingKind::Tracked`]; only
/// `Tracked` dives are ever eligible for upload to SSI. Flip a dive to
/// `Training` to keep it in local history but exclude it from SSI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrackingKind {
    /// A "real" dive intended to be uploaded to SSI.
    #[default]
    Tracked,
    /// A training/practice dive (e.g. pool, course) kept only in local history.
    Training,
}
