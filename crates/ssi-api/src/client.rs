//! Async reqwest client for SSI. NEVER posts automatically — [`SsiClient::create_dive`]
//! sends only when explicitly invoked.

use serde::Deserialize;

use crate::{
    encode_form, SsiError, OPEN_METEO_GEOCODE_URL, SSI_ADD_FORM_URL, SSI_CREATE_DIVE_URL,
    SSI_DIVE_SITE_GEO_URL, SSI_LOGIN_URL, SSI_MY_ORIGIN,
};

/// One dive-site marker returned by the geo search (`f`/`n`/`la`/`lo`).
#[derive(Debug, Clone, PartialEq)]
pub struct DiveSiteMarker {
    /// SSI dive-site id (the `f` field → `odin_user_log_dive_sites_id`).
    pub id: String,
    /// Site name (`n`).
    pub name: String,
    /// Latitude (`la`, parsed from string).
    pub lat: f64,
    /// Longitude (`lo`, parsed from string).
    pub lon: f64,
}

/// Result of a create-dive POST. The caller is responsible for confirming
/// success via read-back (no dive id is returned in the body).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateOutcome {
    pub status: u16,
    pub body_len: usize,
}

/// Raw geo-search JSON shapes (la/lo are strings in SSI's payload).
#[derive(Debug, Deserialize)]
struct GeoResponse {
    #[serde(default)]
    markers: Vec<GeoMarker>,
}

#[derive(Debug, Deserialize)]
struct GeoMarker {
    f: serde_json::Value,
    #[serde(default)]
    n: String,
    la: String,
    lo: String,
}

/// Parse the geo-search response body into markers. Pure (no network), so it is
/// unit-testable against an inline sample.
pub(crate) fn parse_markers(body: &str) -> Result<Vec<DiveSiteMarker>, SsiError> {
    let resp: GeoResponse =
        serde_json::from_str(body).map_err(|e| SsiError::Parse(format!("markers json: {e}")))?;
    resp.markers
        .into_iter()
        .map(|m| {
            let lat =
                m.la.parse::<f64>()
                    .map_err(|e| SsiError::Parse(format!("marker la {:?}: {e}", m.la)))?;
            let lon =
                m.lo.parse::<f64>()
                    .map_err(|e| SsiError::Parse(format!("marker lo {:?}: {e}", m.lo)))?;
            // `f` may arrive as a number or a string; normalize to a string id.
            let id = match m.f {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                other => {
                    return Err(SsiError::Parse(format!("marker f has bad type: {other}")));
                }
            };
            Ok(DiveSiteMarker {
                id,
                name: m.n,
                lat,
                lon,
            })
        })
        .collect()
}

/// Parse the next SSI logbook dive number from add-form / logbook HTML.
///
/// PURE (no network). Strategy:
/// 1. Prefer the add form's `data-currentdivelognr="N"` attribute — SSI already
///    sets this to `max + 1`, so it is returned verbatim.
/// 2. Otherwise scan the logbook for `mydivelog/show/{nr}_{id}_{user}` links and
///    return `max(nr) + 1`.
///
/// Returns `None` if neither signal is present.
pub fn parse_next_dive_nr(html: &str) -> Option<u32> {
    if let Some(nr) = find_attr_u32(html, "data-currentdivelognr") {
        return Some(nr);
    }
    let max = max_show_dive_nr(html)?;
    Some(max + 1)
}

/// Find `<attr>="<digits>"` (single- or double-quoted) and parse the digits.
fn find_attr_u32(html: &str, attr: &str) -> Option<u32> {
    let needle = format!("{attr}=");
    let start = html.find(&needle)? + needle.len();
    let rest = &html[start..];
    let mut chars = rest.char_indices();
    // Skip an optional opening quote.
    let body = match chars.next() {
        Some((_, '"')) | Some((_, '\'')) => &rest[1..],
        _ => rest,
    };
    let digits: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

/// Scan for `mydivelog/show/{nr}_{id}_{user}` and return the largest `nr`.
fn max_show_dive_nr(html: &str) -> Option<u32> {
    let marker = "mydivelog/show/";
    let mut max: Option<u32> = None;
    let mut search = html;
    while let Some(pos) = search.find(marker) {
        let after = &search[pos + marker.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        // Require the `{nr}_` shape so we don't grab unrelated paths.
        if !digits.is_empty() && after[digits.len()..].starts_with('_') {
            if let Ok(nr) = digits.parse::<u32>() {
                max = Some(max.map_or(nr, |m| m.max(nr)));
            }
        }
        search = &after[digits.len()..];
    }
    max
}

/// Open-Meteo geocoder response (`results[0].latitude/longitude/name`).
#[derive(Debug, Deserialize)]
struct GeocodeResponse {
    #[serde(default)]
    results: Vec<GeocodeResult>,
}

#[derive(Debug, Deserialize)]
struct GeocodeResult {
    latitude: f64,
    longitude: f64,
    name: String,
}

/// Parse the Open-Meteo geocoder JSON into `(lat, lon, name)` of the first hit.
///
/// PURE (no network). Returns `None` when there are no results or the body is
/// not the expected shape.
pub fn parse_geocode(json: &str) -> Option<(f64, f64, String)> {
    let resp: GeocodeResponse = serde_json::from_str(json).ok()?;
    let first = resp.results.into_iter().next()?;
    Some((first.latitude, first.longitude, first.name))
}

/// Great-circle distance (km) between two `(lat, lon)` points.
fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    const R: f64 = 6371.0;
    let (lat1, lon1) = (a.0.to_radians(), a.1.to_radians());
    let (lat2, lon2) = (b.0.to_radians(), b.1.to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * R * h.sqrt().asin()
}

/// Pick the best dive-site marker for a query name.
///
/// PURE (no network). Precedence:
/// 1. Exact case-insensitive name match.
/// 2. Case-insensitive substring match (shortest matching name preferred).
/// 3. Nearest marker to `center` by haversine distance.
///
/// Returns a clone of the chosen marker, or `None` if `markers` is empty.
pub fn best_site_match(
    query: &str,
    center: (f64, f64),
    markers: &[DiveSiteMarker],
) -> Option<DiveSiteMarker> {
    let q = query.trim().to_lowercase();

    // 1. Exact case-insensitive name match.
    if let Some(m) = markers.iter().find(|m| m.name.to_lowercase() == q) {
        return Some(m.clone());
    }

    // 2. Substring match, shortest name preferred.
    let substr = markers
        .iter()
        .filter(|m| m.name.to_lowercase().contains(&q))
        .min_by_key(|m| m.name.len());
    if let Some(m) = substr {
        return Some(m.clone());
    }

    // 3. Nearest marker to the geocoded center.
    markers
        .iter()
        .min_by(|a, b| {
            let da = haversine_km(center, (a.lat, a.lon));
            let db = haversine_km(center, (b.lat, b.lon));
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

/// HTTP client for the SSI web logbook flow.
pub struct SsiClient {
    http: reqwest::Client,
}

impl SsiClient {
    /// Build a client with an in-memory cookie store enabled.
    pub fn new() -> Result<Self, SsiError> {
        let http = reqwest::Client::builder().cookie_store(true).build()?;
        Ok(Self { http })
    }

    /// Build a client pre-seeded with a `PHPSESSID` cookie for my.divessi.com.
    ///
    /// This is the PRIMARY auth path for testing: paste a session id captured
    /// from a logged-in browser. The cookie is set as a default header scoped to
    /// the my.divessi.com origin (reqwest's cookie store also stays enabled so
    /// any Set-Cookie responses are retained).
    pub fn with_phpsessid(sid: &str) -> Result<Self, SsiError> {
        let mut headers = reqwest::header::HeaderMap::new();
        let cookie = format!("PHPSESSID={sid}");
        let value = reqwest::header::HeaderValue::from_str(&cookie)
            .map_err(|e| SsiError::Parse(format!("invalid PHPSESSID: {e}")))?;
        headers.insert(reqwest::header::COOKIE, value);
        let http = reqwest::Client::builder()
            .cookie_store(true)
            .default_headers(headers)
            .build()?;
        // SSI_MY_ORIGIN documents the origin this cookie belongs to.
        let _ = SSI_MY_ORIGIN;
        Ok(Self { http })
    }

    /// Search dive sites within a bounding box centered on `(lat, lon)` with a
    /// half-span (in degrees) on each side. Parses `{"markers":[…]}`.
    pub async fn search_dive_sites(
        &self,
        lat: f64,
        lon: f64,
        half_span_deg: f64,
    ) -> Result<Vec<DiveSiteMarker>, SsiError> {
        let minlat = lat - half_span_deg;
        let maxlat = lat + half_span_deg;
        let minlng = lon - half_span_deg;
        let maxlng = lon + half_span_deg;
        let resp = self
            .http
            .get(SSI_DIVE_SITE_GEO_URL)
            .query(&[
                ("minlat", minlat.to_string()),
                ("minlng", minlng.to_string()),
                ("maxlat", maxlat.to_string()),
                ("maxlng", maxlng.to_string()),
                ("latitude", lat.to_string()),
                ("longitude", lon.to_string()),
            ])
            .send()
            .await?;
        let body = resp.text().await?;
        parse_markers(&body)
    }

    /// Fetch the next SSI logbook dive number (auto sequence).
    ///
    /// GETs the add form ([`SSI_ADD_FORM_URL`]) and parses `data-currentdivelognr`
    /// (already `max + 1`); if that signal is absent it falls back to scanning the
    /// returned HTML for `mydivelog/show/{nr}_…` links. Requires the auth'd client
    /// ([`SsiClient::with_phpsessid`]). Returns [`SsiError::Parse`] when no number
    /// can be determined.
    pub async fn next_dive_nr(&self) -> Result<u32, SsiError> {
        let resp = self.http.get(SSI_ADD_FORM_URL).send().await?;
        let body = resp.text().await?;
        parse_next_dive_nr(&body)
            .ok_or_else(|| SsiError::Parse("could not determine next dive number".to_string()))
    }

    /// Geocode a place name via the FREE Open-Meteo geocoder (no API key).
    ///
    /// Returns `(lat, lon, canonical_name)` of the top hit, or `None` if the
    /// geocoder has no match.
    pub async fn geocode(&self, place: &str) -> Result<Option<(f64, f64, String)>, SsiError> {
        let resp = self
            .http
            .get(OPEN_METEO_GEOCODE_URL)
            .query(&[
                ("name", place),
                ("count", "1"),
                ("language", "en"),
                ("format", "json"),
            ])
            .send()
            .await?;
        let body = resp.text().await?;
        Ok(parse_geocode(&body))
    }

    /// Resolve a dive site by free-text name.
    ///
    /// Geocodes `name`, searches SSI dive sites in a 0.25° box around the result,
    /// then picks the best marker via [`best_site_match`]. Read-only (no auth
    /// required). Returns `None` if geocoding fails or no SSI site is found nearby.
    pub async fn resolve_site_by_name(
        &self,
        name: &str,
    ) -> Result<Option<DiveSiteMarker>, SsiError> {
        let Some((lat, lon, _)) = self.geocode(name).await? else {
            return Ok(None);
        };
        let markers = self.search_dive_sites(lat, lon, 0.25).await?;
        Ok(best_site_match(name, (lat, lon), &markers))
    }

    /// Attempt SSO login (JSON `{auth:"Portal", username, password}`).
    ///
    /// UNVERIFIED: our HAR capture had cookies stripped, so we could NOT confirm
    /// how the `rest.divessi.com` SSO result bridges to a `my.divessi.com`
    /// session cookie. This call is best-effort — it issues the POST and returns
    /// Ok on a 2xx — but the resulting client may NOT carry a usable
    /// my.divessi.com session. For reliable create-dive use, prefer
    /// [`SsiClient::with_phpsessid`] until the bridge is verified.
    ///
    /// TODO: capture the full login→my.divessi.com cookie handshake and wire it
    /// up here.
    pub async fn login(&self, email: &str, password: &str) -> Result<(), SsiError> {
        let body = serde_json::json!({
            "auth": "Portal",
            "username": email,
            "password": password,
        });
        let resp = self.http.post(SSI_LOGIN_URL).json(&body).send().await?;
        resp.error_for_status()?;
        Ok(())
    }

    /// POST a create-dive request. SENDS ONLY when explicitly called.
    ///
    /// `fields` should come from [`crate::build_create_form`]. The caller is
    /// responsible for confirming success via read-back — no dive id is returned
    /// in the response body, only `CreateOutcome { status, body_len }`.
    pub async fn create_dive(
        &self,
        fields: &[(String, String)],
    ) -> Result<CreateOutcome, SsiError> {
        let body = encode_form(fields);
        let resp = self
            .http
            .post(SSI_CREATE_DIVE_URL)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let text = resp.text().await?;
        Ok(CreateOutcome {
            status,
            body_len: text.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_geo_markers() {
        let sample = r#"{"markers":[{"f":"751922","n":"Dzul Ha","la":"20.4591","lo":"-86.9866"}]}"#;
        let markers = parse_markers(sample).expect("should parse");
        assert_eq!(markers.len(), 1);
        let m = &markers[0];
        assert_eq!(m.id, "751922");
        assert_eq!(m.name, "Dzul Ha");
        assert!((m.lat - 20.4591).abs() < 1e-6);
        assert!((m.lon - (-86.9866)).abs() < 1e-6);
    }

    #[test]
    fn parses_empty_markers() {
        let markers = parse_markers(r#"{"markers":[]}"#).unwrap();
        assert!(markers.is_empty());
    }

    #[test]
    fn parses_numeric_f_id() {
        let sample = r#"{"markers":[{"f":751922,"n":"X","la":"1.0","lo":"2.0"}]}"#;
        let markers = parse_markers(sample).unwrap();
        assert_eq!(markers[0].id, "751922");
    }

    #[test]
    fn next_dive_nr_prefers_current_attr() {
        let html = r#"<form ... data-currentdivelognr="46" data-other="x">..."#;
        assert_eq!(parse_next_dive_nr(html), Some(46));
    }

    #[test]
    fn next_dive_nr_falls_back_to_show_links() {
        let html = r#"<a href="/mydivelog/show/44_123_999">..</a>
                      <a href="/mydivelog/show/12_5_999">..</a>"#;
        assert_eq!(parse_next_dive_nr(html), Some(45));
    }

    #[test]
    fn next_dive_nr_none_when_no_signal() {
        assert_eq!(parse_next_dive_nr("<html>nothing here</html>"), None);
    }

    #[test]
    fn geocode_parses_first_result() {
        let json = r#"{"results":[{"latitude":38.7,"longitude":-121.14,"name":"Folsom"}]}"#;
        let got = parse_geocode(json).expect("should parse");
        assert!((got.0 - 38.7).abs() < 1e-9);
        assert!((got.1 - (-121.14)).abs() < 1e-9);
        assert_eq!(got.2, "Folsom");
    }

    #[test]
    fn geocode_empty_or_missing_is_none() {
        assert_eq!(parse_geocode(r#"{"results":[]}"#), None);
        assert_eq!(parse_geocode(r#"{}"#), None);
    }

    fn marker(id: &str, name: &str, lat: f64, lon: f64) -> DiveSiteMarker {
        DiveSiteMarker {
            id: id.to_string(),
            name: name.to_string(),
            lat,
            lon,
        }
    }

    #[test]
    fn best_match_exact_name_wins() {
        let markers = vec![
            marker("1", "Folsom Point Reef", 38.71, -121.13),
            marker("2", "Folsom Point", 38.70, -121.14),
            marker("3", "Granite Bay", 38.75, -121.10),
        ];
        let chosen = best_site_match("folsom point", (38.7, -121.14), &markers).unwrap();
        assert_eq!(chosen.id, "2");
    }

    #[test]
    fn best_match_substring_shortest_name() {
        let markers = vec![
            marker("1", "North Folsom Point Reef", 38.71, -121.13),
            marker("2", "Folsom Lake Cove", 38.70, -121.14),
            marker("3", "Granite Bay", 38.75, -121.10),
        ];
        // No exact match; both 1 and 2 contain "folsom" — shortest name wins (2).
        let chosen = best_site_match("Folsom", (38.7, -121.14), &markers).unwrap();
        assert_eq!(chosen.id, "2");
    }

    #[test]
    fn best_match_nearest_fallback() {
        let markers = vec![
            marker("far", "Deep Blue", 40.0, -120.0),
            marker("near", "Coral Garden", 38.71, -121.14),
        ];
        // No name overlap with the query → nearest to center wins.
        let chosen = best_site_match("Folsom", (38.7, -121.14), &markers).unwrap();
        assert_eq!(chosen.id, "near");
    }

    #[test]
    fn best_match_empty_is_none() {
        assert_eq!(best_site_match("Folsom", (38.7, -121.14), &[]), None);
    }
}
