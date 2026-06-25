# crate: divebridge-enrich

**Role:** Augment a `core::Dive` with weather/ocean data and suggested tags/type.
**Deterministic by default; AI is optional and fail-safe.**

## Provider trait
```
trait EnrichProvider {
    fn enrich(&self, dive: &Dive) -> Result<Enrichment, EnrichError>;
}
```
- `DataFeedProvider` (default): Open-Meteo Historical Weather (-> `Weather`),
  Open-Meteo Marine + NOAA CO-OPS (-> `Ocean`), profile-shape heuristics for
  dive type/tags. Needs dive site GPS + start time. No keys.
- `ClaudeCliProvider` (OPTIONAL): shells out to local `claude -p … --output-format
  json` (or MCP/ACP). **MUST fail gracefully** — if `claude` is missing/unreachable,
  return `Ok(Enrichment::default())` or a soft error the caller ignores; never block
  the pipeline. For Ken to experiment with; not a dependency.

## Invariants
- Enrichment writes only into `DiveLog` (editable overlay); never the raw layer.
- Human review before any value is sent to SSI.

## Dependencies
divebridge-core, reqwest (json, pending), serde, serde_json, thiserror.
