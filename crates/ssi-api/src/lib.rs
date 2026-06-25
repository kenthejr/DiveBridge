//! Direct SSI HTTP client — see `CONTRACT.md`. [stub]
//!
//! Endpoints CONFIRMED via HAR capture (2026-06-25); see docs/ssi-integration.md.

pub use divebridge_core as core;

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
