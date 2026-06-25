//! Direct SSI HTTP client — see `CONTRACT.md`. [stub]

pub use divebridge_core as core;

/// Known SSI API base (see docs/ssi-integration.md). Auth & read endpoints are
/// confirmed; the create endpoint is discovered in Spike 1.
pub const SSI_API_BASE: &str = "https://api.divessi.com/app/a21.php";
/// Hardcoded client identifier observed in the wild.
pub const SSI_APP_ID: &str = "0815_ADR";
