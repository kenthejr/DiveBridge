//! SI units. Everything in `core` is stored in SI; conversion to/from imperial
//! or device-native units happens at the edges (display, ingest, SSI submit).

use serde::{Deserialize, Serialize};

macro_rules! f64_newtype {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {
        $(
            $(#[$m])*
            #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
            pub struct $name(pub f64);
        )*
    };
}

f64_newtype!(
    /// Depth or distance in meters.
    Meters,
    /// Temperature in degrees Celsius.
    Celsius,
    /// Pressure in bar.
    Bar,
    /// Mass in kilograms.
    Kilograms,
    /// Volume in liters.
    Liters,
);

/// A duration in whole seconds (dive times are integer-second resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Seconds(pub i64);

impl Seconds {
    pub fn minutes(&self) -> f64 {
        self.0 as f64 / 60.0
    }
}
