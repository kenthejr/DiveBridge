# SSI integration

**Confirmed by HAR capture (2026-06-25)** of logging one dive on my.divessi.com.
The live web logbook uses the **legacy MySSI 2017 web flow** (form POST + session
cookie), NOT the `a21.php` app API. Direct submission is viable.

## Auth (confirmed)
- `POST https://rest.divessi.com/sso/login`
  - Content-Type: `application/json`
  - Body: `{"auth":"Portal","username":"<email>","password":"<password>"}`
  - On success sets session cookies used for subsequent my.divessi.com calls.
  - (CORS preflight `OPTIONS` precedes it.)
- After login, the account's `user_master_id` (a number, e.g. the value posted as
  `odin_user_log_user_master_id`) identifies the user. Obtain it from the profile/
  session (it appears in page context); needed on every create.

## Create dive (confirmed) — PRIMARY upload path
- `POST https://my.divessi.com/code/process/mydivelog_18.php`
  - Content-Type: `application/x-www-form-urlencoded`; **session cookie required**.
  - ~88 fields (see `crates/ssi-api/tests/fixtures/create-dive.request.txt`,
    sanitized). Constants: `source=mydl_18_add_AddDiveOnline`, `submit=Submit`.
  - Returns `200 text/html` (success page/redirect).
- Pre-check: `POST .../code/process/ajax_divelog_validate_dive_number.php` (same
  field set, returns JSON) validates `odin_user_log_dive_nr` availability.

## Dive-site resolution (confirmed) — it's a SEARCH, not a dropdown
- `GET .../code/geo/dive_site.json.sd.php?minlat&minlng&maxlat&maxlng&latitude&
  longitude` → `{"markers":[ {f,n,m,t,la,lo,p,lck,ld,d,otype}, … ]}` where:
  - **`f` = dive site id** → this is `odin_user_log_dive_sites_id`.
  - `n` = name, `la`/`lo` = lat/lon, `lck`=1 (locked site), `otype`=`site_locked`.
  - Cozumel bbox returned 81 markers; **Folsom returned `{"markers":[]}`**.
- **Resolution flow:** dive GPS (from enrichment/Garmin/manual; Perdix has none) →
  bbox around it → query endpoint → choose nearest / name-matching marker → use `f`.
  - **Fallback when empty** (no SSI-registered site, e.g. Folsom): widen bbox / let
    user pick a nearby site, or submit with site blank + put the location name in
    `comment`. Don't fabricate a site id. (Creating a new SSI site is out of scope.)
  - Text/address search in the UI uses Google geocoding (`maps.googleapis.com`) →
    coords → same endpoint; `searchSite`/`adr` form fields feed that. `dive_site_bow`
    = body of water (e.g. `fresh`).
- `ds_infowindow_sd.php` returns a site info popup (needs more than just an id;
  returned ERROR with id alone — not needed, we have `f`).
- `ajax_divearea_getAll.php` → big **region polygons** (WKT), not individual sites;
  not used for resolution.

## Other "search" fields
- **Buddies** (`odin_user_log_buddy_ids[]`) and **facility** (`log_linked_facility_id`)
  render in the add form as selects pre-populated with the ACCOUNT's existing
  buddies/facilities — so DiveBridge matches a buddy by name against that account
  list (fetched live). Searching the *global* SSI member/facility DB to add a NEW
  buddy is a separate edge case (defer).

## Field map (key fields → core::Dive)
- Date/time: `date_sel2_dd / _mm / _yy`, `odin_user_log_entry_time` ("HH:MM").
- Identity: `odin_user_log_dive_nr` = **SSI logbook sequence** (NOT the computer
  dive number — they differ: capture had SSI nr 45 vs Perdix #42).
- Depth/time: `odin_user_log_divetime`(min), `_depth_m`/`_depth_ft`,
  `_avg_depth_m`/`_ft`.
- Gas/tank: `_ean`(0/1), `_ean_percent`, `_var_tanktype_id`, `_tank_vol_l`/`_cuft`,
  `_pressure_start_bar`/`_psi`, `_pressure_end_bar`/`_psi`, `_amv_l`/`_psi`.
- Temp: `_airtemp_c`/`_f`, `_watertemp_c`/`_f`, `_watertemp_max_c`/`_f`.
- Conditions/vis: `_vis_m`/`_ft`, plus enumerated `var_*` ids below.
- Site/buddy/facility: `_dive_sites_id`, `dive_site_bow`, `_buddy_ids[]`,
  `_leader_nr`, `log_linked_facility_id`.
- Free text: `_gear_details`, `_comment`. Rating: `_rating` (1–5). Weight:
  `_weight_kg`/`_lb`.
- Dive-computer attach (EMPTY in manual capture, but present): `_diveComputer`,
  `_diveComputerData_ue`, `_divecomputer_ref`, `_divecomputer_dive_ref`,
  `_divecomputer_imported`, `_transferDate`. → likely the channel for attaching
  Perdix profile data. Investigate in Round 2 (capture an app/import flow).

## Enumerated vocabularies (IDs we must map)
`odin_user_log_var_divetype_id`, `_var_entry_id`, `_var_water_body_id`,
`_var_watertype_id`, `_var_current_id`, `_var_surface_id`, `_var_weather_id`,
`_var_specialdive_id[]` (multi), `odin_user_log_animal_ids` (wildlife),
`_gearconfiguration_id`, `_dive_type`. Need the value→label tables (from the add
form's `<select>` options or languagepack/modelproperties.php). TODO: capture the
GET `mydivelog/add` form HTML + `api/www/modelproperties.php` to extract these.

## Units note
The form sends BOTH metric and imperial. We store SI; populate `_m/_c/_bar/_kg/_l`
from core and compute the `_ft/_f/_psi/_lb/_cuft` siblings (or leave blank and let
the server derive — verify which).

## Legacy alt (read API)
`GET https://api.divessi.com/app/a21.php?...&what=authenticate|get_divelog&
ssiapp=0815_ADR` — token API, useful for READ-BACK verification. Not used for create.

## Round 2 (open)
- SSI dive_nr assignment (next-in-sequence) and the validate endpoint's response.
- Vocabulary tables (extract from form/modelproperties).
- Dive-computer attach fields — can we upload the Perdix profile (graphs) too?
- Idempotency: no obvious dive UUID returned; read-back via a21.php `get_divelog`
  to detect dupes; store `SyncState.remote_id`.
- Edit/update vs create (only create observed).
