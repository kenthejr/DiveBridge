//! Async reqwest client for SSI. NEVER posts automatically — [`SsiClient::create_dive`]
//! sends only when explicitly invoked.

use serde::Deserialize;

use crate::{
    encode_form, SsiError, SSI_CREATE_DIVE_URL, SSI_DIVE_SITE_GEO_URL, SSI_LOGIN_URL, SSI_MY_ORIGIN,
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
}
