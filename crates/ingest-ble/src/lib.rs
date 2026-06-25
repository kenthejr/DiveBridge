//! Shearwater BLE -> core::Dive adapter — see `CONTRACT.md`. [stub]
//!
//! The protocol/transport lives in the separate `shearwater-protocol` /
//! `shearwater-ble` crates (dual MIT/Apache, public repo). This crate only maps
//! their output into the DiveBridge domain model.

pub use divebridge_core as core;
