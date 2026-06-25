//! DiveBridge app shell. Currently a placeholder entrypoint; becomes the Tauri v2
//! backend (commands wiring ingest/store/ssi/enrich) once the vertical slice lands.

use divebridge_core::TrackingKind;

fn main() {
    println!(
        "DiveBridge {} — scaffolding. New dives default to {:?}. See docs/ and the plan.",
        env!("CARGO_PKG_VERSION"),
        TrackingKind::default()
    );
}
