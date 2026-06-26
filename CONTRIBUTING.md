# Contributing to DiveBridge

Thanks for your interest in DiveBridge! Contributions — bug reports, fixes,
dive-computer support, SSI mapping improvements — are welcome.

> Status: early development. APIs and internals are still moving; open an issue to
> discuss anything substantial before a large PR.

## Getting started

```sh
git clone git@github.com:kenthejr/DiveBridge.git
cd DiveBridge
cargo build
cargo test --workspace
```

Requires a recent stable Rust (see `rust-toolchain.toml`, 1.90+). The workspace is
a set of small crates — see [`docs/`](docs/) and each crate's `CONTRACT.md` for the
design. Read [`docs/parallelization-playbook.md`](docs/parallelization-playbook.md)
for how the codebase is organized.

## Before you open a PR

- `cargo fmt --all` — formatting must be clean.
- `cargo clippy --workspace --all-targets -- -D warnings` — no warnings.
- `cargo test --workspace` — all tests green. Add tests for new behavior;
  parsers and mappers should be covered by fixture-based unit tests (no live
  network in tests).
- Keep changes scoped to one crate where possible. **`divebridge-core` is the
  frozen domain model** — changes there are deliberate and reviewed separately, not
  a side effect of feature work.
- Match the surrounding style; avoid `unwrap()` on fallible IO/parse paths (return
  typed errors).

## Privacy & safety

DiveBridge handles personal dive data and account credentials. Never commit
captures (`.har`), session cookies, credentials, or real personal data. Test
fixtures must be sanitized. The SSI client must never auto-submit or attach a dive
center without explicit user action.

## Commits

- Write clear, imperative commit messages.
- Signed commits are appreciated (the project uses SSH commit signing).

## License of contributions

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as below, without any additional terms or conditions.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.
