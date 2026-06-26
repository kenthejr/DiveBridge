//! PURE mapping from [`core::Dive`] (+ UI-supplied [`SubmitContext`]) to the SSI
//! create-dive form fields, and a urlencoded serializer for that field list.
//!
//! The field set/order mirrors the sanitized real request in
//! `tests/fixtures/create-dive.request.txt`. We emit empty strings for fields we
//! don't populate so the request body shape matches the real form.

use crate::core;
use crate::{SsiError, SSI_CREATE_SOURCE};
use chrono::{Datelike, Timelike};

/// Non-dive inputs the UI supplies at submit time (vocab ids, sequence number,
/// account identity, resolved site, buddies, …). Kept separate from `core::Dive`
/// because none of this lives in the frozen domain model.
#[derive(Debug, Clone, Default)]
pub struct SubmitContext {
    /// The account's `user_master_id` (from the add-form / session).
    pub user_master_id: String,
    /// SSI logbook sequence number (NOT the computer's own dive number).
    pub dive_nr: u32,
    /// Resolved SSI dive-site id (`f` from the geo search), if any.
    pub dive_sites_id: Option<String>,
    /// Body of water: `"fresh"` | `"salt"`.
    pub dive_site_bow: Option<String>,
    /// Primary dive type vocab id (default Fun Dive = "24").
    pub var_divetype_id: Option<String>,
    /// Entry vocab id (Shore/Beach, Boat, …).
    pub var_entry_id: Option<String>,
    /// Water body vocab id (Ocean, Lake, …).
    pub var_water_body_id: Option<String>,
    /// Water type vocab id (Fresh/Salt).
    pub var_watertype_id: Option<String>,
    /// Current vocab id.
    pub var_current_id: Option<String>,
    /// Surface conditions vocab id.
    pub var_surface_id: Option<String>,
    /// Weather vocab id.
    pub var_weather_id: Option<String>,
    /// Tank-type vocab id (Steel/Aluminum).
    pub var_tanktype_id: Option<String>,
    /// Gear configuration vocab id.
    pub gearconfiguration_id: Option<String>,
    /// Special-dive (tag) vocab ids → repeated `odin_user_log_var_specialdive_id[]`.
    pub specialdive_ids: Vec<String>,
    /// Account buddy ids → repeated `odin_user_log_buddy_ids[]`.
    pub buddy_ids: Vec<String>,
}

// Imperial conversion factors (validated against the fixture).
const M_PER_FT: f64 = 0.3048;
const LB_PER_KG: f64 = 2.2046226;
const PSI_PER_BAR: f64 = 14.5037738;

fn m_to_ft(m: f64) -> i64 {
    (m / M_PER_FT).round() as i64
}
fn kg_to_lb(kg: f64) -> i64 {
    (kg * LB_PER_KG).round() as i64
}
fn bar_to_psi(bar: f64) -> i64 {
    (bar * PSI_PER_BAR).round() as i64
}
fn c_to_f(c: f64) -> i64 {
    (c * 9.0 / 5.0 + 32.0).round() as i64
}

/// Render an f64 without a trailing `.0` for whole numbers (so `13.7` stays
/// `13.7` but `8.0` becomes `8`), matching the metric values in the fixture.
fn fmt_num(v: f64) -> String {
    if (v.fract()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        // Trim trailing zeros, keep up to a sensible precision.
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}

/// Build the ordered list of `(name, value)` form fields for an SSI create-dive
/// request. Pure: no IO, no clock, fully determined by `dive` + `ctx`.
///
/// Returns [`SsiError::NotUploadable`] for non-`Tracked` dives.
pub fn build_create_form(
    dive: &core::Dive,
    ctx: &SubmitContext,
) -> Result<Vec<(String, String)>, SsiError> {
    if !dive.is_uploadable() {
        return Err(SsiError::NotUploadable);
    }

    let s = &dive.summary;
    let log = &dive.log;

    // --- Date / time (UTC) ---
    let start = s.start;
    let dd = format!("{:02}", start.day());
    let mm = format!("{:02}", start.month());
    let yy = format!("{:04}", start.year());
    let entry_time = format!("{:02}:{:02}", start.hour(), start.minute());

    // --- Bottom time (minutes, rounded) ---
    let divetime = s.total_bottom_time.minutes().round() as i64;

    // --- Depth ---
    let depth_m = s.max_depth.0;
    let depth_ft = m_to_ft(depth_m);
    let avg_depth_m = s.avg_depth.map(|d| d.0);

    // --- Temperatures ---
    let watertemp_c = s.min_temp.map(|c| c.0);
    let airtemp_c = log.weather.as_ref().and_then(|w| w.air_temp_c);

    // --- Pressures ---
    let pressure_start_bar = s.pressure_start.map(|b| b.0);
    let pressure_end_bar = s.pressure_end.map(|b| b.0);

    // --- Gas / nitrox: primary source's first gas mix ---
    let primary_gas = dive
        .primary()
        .and_then(|src| src.gases.first().copied())
        .or_else(|| s.gases.first().copied());
    let (ean, ean_percent) = match primary_gas {
        Some(g) if g.o2_percent > 21.5 => ("1".to_string(), format!("{}", g.o2_percent.round())),
        _ => ("0".to_string(), String::new()),
    };

    // --- From the editable DiveLog ---
    let comment = log.notes.clone().unwrap_or_default();
    let vis_m = log.visibility.map(|v| v.0);
    let weight_kg = log.weight.map(|w| w.0);

    // --- Context defaults ---
    let var_divetype_id = ctx
        .var_divetype_id
        .clone()
        .unwrap_or_else(|| "24".to_string());

    // Helper to render an optional metric value + its imperial sibling.
    let opt_m = |m: Option<f64>| m.map(fmt_num).unwrap_or_default();
    let opt_ft = |m: Option<f64>| m.map(|v| m_to_ft(v).to_string()).unwrap_or_default();

    let mut f: Vec<(String, String)> = Vec::new();
    let mut push = |k: &str, v: String| f.push((k.to_string(), v));

    // The order mirrors the fixture's field list (the canonical set).
    push("odin_user_log_user_master_id", ctx.user_master_id.clone());
    push("source", SSI_CREATE_SOURCE.to_string());
    push("odin_user_log_animal_ids", String::new());
    push("odin_user_log_transferDate", String::new());
    push("odin_user_log_diveComputer", String::new());
    push("odin_user_log_diveComputerData_ue", String::new());
    push("odin_user_log_si_before", String::new());
    push("odin_user_log_gf_set", String::new());
    push("odin_user_log_gf_set_1", String::new());
    push("odin_user_log_gf_set_2", String::new());
    push("odin_user_log_gf_end", String::new());
    push("odin_user_log_cns_start", String::new());
    push("odin_user_log_cns_end", String::new());
    push("odin_user_log_otu_start", String::new());
    push("odin_user_log_otu_end", String::new());
    push("odin_user_log_alarm_deco_stop", String::new());
    push("odin_user_log_alarm_fast_ascent", String::new());
    push("odin_user_log_alarm_deco_violation", String::new());
    push("odin_user_log_divecomputer_dive_ref", String::new());
    push("odin_user_log_divecomputer_ref", String::new());
    push("odin_user_log_divecomputer_imported", String::new());
    push("odin_user_log_dive_type", "0".to_string());
    push("date_sel2_dd", dd);
    push("date_sel2_mm", mm);
    push("date_sel2_yy", yy);
    push("odin_user_log_entry_time", entry_time);
    push("odin_user_log_dive_nr", ctx.dive_nr.to_string());
    push("odin_user_log_var_divetype_id", var_divetype_id);
    push("log_linked_brevet_rule_id", "0".to_string());
    // Repeated buddy ids — emitted below as multiple keys.
    for id in &ctx.buddy_ids {
        push("odin_user_log_buddy_ids[]", id.clone());
    }
    push("odin_user_log_leader_nr", String::new());
    // SAFETY: never link a facility on a (potentially test) submission.
    push("log_linked_facility_id", String::new());
    push(
        "odin_user_log_dive_sites_id",
        ctx.dive_sites_id.clone().unwrap_or_default(),
    );
    push(
        "dive_site_bow",
        ctx.dive_site_bow.clone().unwrap_or_default(),
    );
    push("adr", String::new());
    push("searchSite", String::new());
    push("odin_user_log_divetime", divetime.to_string());
    push("odin_user_log_depth_m", fmt_num(depth_m));
    push("odin_user_log_depth_ft", depth_ft.to_string());
    push("odin_user_log_avg_depth_m", opt_m(avg_depth_m));
    push("odin_user_log_avg_depth_ft", opt_ft(avg_depth_m));
    push("odin_user_log_weight_kg", opt_m(weight_kg));
    push(
        "odin_user_log_weight_lb",
        weight_kg
            .map(|w| kg_to_lb(w).to_string())
            .unwrap_or_default(),
    );
    push("odin_user_log_ean", ean);
    push("odin_user_log_ean_percent", ean_percent);
    push(
        "odin_user_log_gearconfiguration_id",
        ctx.gearconfiguration_id.clone().unwrap_or_default(),
    );
    push(
        "odin_user_log_var_tanktype_id",
        ctx.var_tanktype_id.clone().unwrap_or_default(),
    );
    push("odin_user_log_tank_vol_l", String::new());
    // TODO verify-on-submit: nominal tank size (cuft), not a unit conversion.
    push("odin_user_log_tank_vol_cuft", String::new());
    push(
        "odin_user_log_pressure_start_bar",
        opt_m(pressure_start_bar),
    );
    push(
        "odin_user_log_pressure_start_psi",
        pressure_start_bar
            .map(|b| bar_to_psi(b).to_string())
            .unwrap_or_default(),
    );
    push("odin_user_log_pressure_end_bar", opt_m(pressure_end_bar));
    push(
        "odin_user_log_pressure_end_psi",
        pressure_end_bar
            .map(|b| bar_to_psi(b).to_string())
            .unwrap_or_default(),
    );
    push("odin_user_log_amv_l", String::new());
    // TODO verify-on-submit: AMV is nuanced (not a simple unit conversion).
    push("odin_user_log_amv_psi", String::new());
    push("odin_user_log_deco_time", String::new());
    push("odin_user_log_deco_gas_tanktype_id", String::new());
    push("odin_user_log_deco_gas_tank_vol_l", String::new());
    push("odin_user_log_deco_gas_tank_vol_cuft", String::new());
    push("odin_user_log_deco_gas_o2", String::new());
    push("odin_user_log_deco_gas_start_bar", String::new());
    push("odin_user_log_deco_gas_start_psi", String::new());
    push("odin_user_log_deco_gas_end_bar", String::new());
    push("odin_user_log_deco_gas_end_psi", String::new());
    push("log_extended_data_cleanup_weight_kg", String::new());
    push("log_extended_data_cleanup_weight_lb", String::new());
    // Repeated special-dive (tag) ids.
    for id in &ctx.specialdive_ids {
        push("odin_user_log_var_specialdive_id[]", id.clone());
    }
    push("odin_user_log_rating", String::new());
    push(
        "odin_user_log_var_water_body_id",
        ctx.var_water_body_id.clone().unwrap_or_default(),
    );
    push(
        "odin_user_log_var_entry_id",
        ctx.var_entry_id.clone().unwrap_or_default(),
    );
    push(
        "odin_user_log_var_watertype_id",
        ctx.var_watertype_id.clone().unwrap_or_default(),
    );
    push(
        "odin_user_log_var_current_id",
        ctx.var_current_id.clone().unwrap_or_default(),
    );
    push(
        "odin_user_log_var_surface_id",
        ctx.var_surface_id.clone().unwrap_or_default(),
    );
    push(
        "odin_user_log_var_weather_id",
        ctx.var_weather_id.clone().unwrap_or_default(),
    );
    push("odin_user_log_airtemp_c", opt_m(airtemp_c));
    push(
        "odin_user_log_airtemp_f",
        airtemp_c.map(|c| c_to_f(c).to_string()).unwrap_or_default(),
    );
    push("odin_user_log_watertemp_c", opt_m(watertemp_c));
    push(
        "odin_user_log_watertemp_f",
        watertemp_c
            .map(|c| c_to_f(c).to_string())
            .unwrap_or_default(),
    );
    push("odin_user_log_watertemp_max_c", String::new());
    push("odin_user_log_watertemp_max_f", String::new());
    push("odin_user_log_vis_m", opt_m(vis_m));
    push("odin_user_log_vis_ft", opt_ft(vis_m));
    push("odin_user_log_gear_details", String::new());
    push("odin_user_log_comment", comment);
    push("submit", "Submit".to_string());

    Ok(f)
}

/// Serialize a field list as `application/x-www-form-urlencoded`.
///
/// Repeated keys (e.g. `odin_user_log_buddy_ids[]`,
/// `odin_user_log_var_specialdive_id[]`) are emitted once per entry, as the SSI
/// form expects.
pub fn encode_form(fields: &[(String, String)]) -> String {
    let mut ser = form_urlencoded::Serializer::new(String::new());
    for (k, v) in fields {
        ser.append_pair(k, v);
    }
    ser.finish()
}
