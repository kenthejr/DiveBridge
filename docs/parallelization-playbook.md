# Parallelization playbook

Goal: most work runs as parallel agents, each under ~100k tokens, by working
against contracts + fixtures instead of the whole codebase.

## Rules
1. **Contracts over code.** An agent assigned a crate reads: this playbook + that
   crate's `CONTRACT.md` + `crates/core` (types) + the one `docs/*.md` it touches.
   It does NOT read sibling crate internals.
2. **`core` is frozen.** Changes to `divebridge-core` are a separate, deliberate
   task — never a side effect of crate work. If a crate needs a new shared type,
   flag it; don't fork the model.
3. **Fixtures are the integration glue.** Real captured artifacts let agents test in
   isolation:
   - `crates/ingest-file/tests/fixtures/*.uddf` (real Shearwater export)
   - `shearwater-protocol` byte-dump fixtures (raw BLE frames)
   - `crates/ssi-api/tests/fixtures/*.json` (captured create request/response)
   - `core::Dive` JSON fixtures shared across consumers
4. **One agent per crate/spike.** Structured prompt + structured return. Keep each
   self-contained.
5. **Verify in isolation.** `cargo test -p <crate>` must pass without live hardware
   or network (mock/fixture).

## Current parallelizable units
- Spike 1 (ssi discovery) ‖ Spike 2 (shearwater repo) ‖ Spike 3 (UDDF) — independent.
- After core: `store`, `ingest-file`, `enrich` can proceed in parallel; `ssi-api`
  waits on Spike 1; `ingest-ble` waits on Spike 2.

## Continuous improvement
At each phase boundary ask: "should this be a script or a skill?" Candidates:
contract scaffolder, fixture generator, `core::Dive` round-trip checker, SSI request
replayer, `claude -p` JSON wrapper. Add to `.claude/` when they prove repetitive.
