//! Direct SSI HTTP client — see `CONTRACT.md`.
//!
//! Endpoints CONFIRMED via HAR capture (2026-06-25); see docs/ssi-integration.md.
//!
//! This crate provides two things:
//!
//! 1. A **pure** mapping ([`build_create_form`]) from a [`core::Dive`] +
//!    [`SubmitContext`] to the exact `application/x-www-form-urlencoded` field
//!    list SSI's legacy web logbook expects, plus [`encode_form`] to serialize
//!    it (correctly handling repeated keys).
//! 2. An async [`SsiClient`] (reqwest) that can search dive sites, attempt
//!    login, and POST a create-dive request.
//!
//! ## SAFETY
//! Nothing here POSTs automatically. [`SsiClient::create_dive`] only sends when
//! you explicitly call it. The mapping NEVER sets `log_linked_facility_id`
//! (left blank) so a test submission cannot notify a dive center.

pub use divebridge_core as core;

mod client;
mod mapping;

pub use client::{CreateOutcome, DiveSiteMarker, SsiClient};
pub use mapping::{build_create_form, encode_form, SubmitContext};

/// Login (JSON `{auth:"Portal", username, password}`) → sets session cookies.
pub const SSI_LOGIN_URL: &str = "https://rest.divessi.com/sso/login";
/// Create-dive submission (form-urlencoded, session cookie required). PRIMARY path.
pub const SSI_CREATE_DIVE_URL: &str = "https://my.divessi.com/code/process/mydivelog_18.php";
/// Pre-validate `odin_user_log_dive_nr` (returns JSON).
pub const SSI_VALIDATE_DIVE_NR_URL: &str =
    "https://my.divessi.com/code/process/ajax_divelog_validate_dive_number.php";
/// Dive-site geo search (bounding box) → site ids.
pub const SSI_DIVE_SITE_GEO_URL: &str = "https://my.divessi.com/code/geo/dive_site.json.sd.php";
/// Constant `source` value the add-dive form posts.
pub const SSI_CREATE_SOURCE: &str = "mydl_18_add_AddDiveOnline";

/// Legacy token "app" API — useful for READ-BACK verification only.
pub const SSI_LEGACY_API_BASE: &str = "https://api.divessi.com/app/a21.php";
/// Hardcoded client id used by the legacy app API.
pub const SSI_LEGACY_APP_ID: &str = "0815_ADR";

/// Base origin that owns the my.divessi.com session cookie.
pub(crate) const SSI_MY_ORIGIN: &str = "https://my.divessi.com";

/// Errors surfaced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum SsiError {
    /// A transport-level / reqwest error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// A response body could not be parsed as expected.
    #[error("parse error: {0}")]
    Parse(String),
    /// The dive is not eligible for upload (only `Tracked` dives are).
    #[error("dive is not uploadable (only Tracked dives may be submitted)")]
    NotUploadable,
    /// A required value was missing from the dive/context.
    #[error("missing required value: {0}")]
    Missing(String),
}
