//! File ingestion (UDDF/XML/CSV) -> `core::SourceRecording` / `core::Dive`.
//!
//! See `CONTRACT.md`. Spike 3 implements the Shearwater Cloud UDDF dialect; CSV
//! and Subsurface XML are future work.

pub use divebridge_core as core;
use divebridge_core::units;

mod uddf;

pub use uddf::{kelvin_to_c, pa_to_bar, parse_uddf, parse_uddf_at, to_dives};

/// Errors raised while ingesting an export file.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    /// The file was not valid UTF-8 text.
    #[error("input was not valid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// The XML was malformed or could not be read.
    #[error("malformed XML: {0}")]
    Xml(#[from] quick_xml::Error),

    /// A required element was absent from an otherwise well-formed document.
    #[error("missing required element: {0}")]
    Missing(String),
}
