# SSI integration

SSI has no official API. There is an **undocumented** app API; the read path is
known, the **write path must be discovered** (Spike 1).

## Known surface (from research)
- Base: `https://api.divessi.com/app/a21.php`
- Client id: `ssiapp=0815_ADR`
- **Auth:** `GET ?l=<email>&p=<pass>&what=authenticate&ssiapp=0815_ADR`
  → `{"token": "<token>"}`. Returns HTTP 200 even on bad creds → detect by missing
  `token`.
- **Read dives:** `GET ?what=get_divelog&token=<token>&ssiapp=0815_ADR`
  → `{ logbook_sites:[{odin_dive_sites_id, odin_dive_sites_name}],
       logbook_details:[{odin_user_log_*}] }`
- Dive fields (`odin_user_log_*`): `nr, date, entry_time, divetime, depth_m,
  watertemp_c, watertemp_max_c, weight_kg, var_tanktype_id, tank_vol_l, ean,
  ean_percent, pressure_start_bar, pressure_end_bar, avg_depth_m, amv_l,
  gear_details, divecenter_confirmed_name, comment, dive_sites_id`.

## TODO — Spike 1 (browser-first)
Capture my.divessi.com traffic while manually logging one dive; record:
- [ ] create-dive request: method, URL, params/body, exact write field names
- [ ] how dive sites are searched/selected (site id source)
- [ ] auth/session shape used by the web app (vs the `a21.php` token)
- [ ] save a request fixture under `crates/ssi-api/tests/fixtures/`

If the browser doesn't reveal a clean direct call, escalate to mitmproxy on the
MySSI mobile app (Ken can run with direction).

## Mapping notes (core::Dive → SSI), to finalize in Round 2
- SI values map directly to `*_m / *_c / *_bar / *_kg`.
- Multi-gas dive → primary/bottom gas into `ean`/`ean_percent`; extras into comment.
- `descent_count`/segments have no SSI field → summarize in comment.
- Resolve `DiveSite.ssi_site_id` via site search; cache name→id.
- Idempotency: `Dive::content_hash()` + `SyncState`; skip synced-unchanged.
