# Agents

## Setup and checks

Follow [CONTRIBUTING.md](CONTRIBUTING.md#development-setup). Use the pinned nightly from `rust-toolchain.toml`; `+stable` is only for installing development tools.

Before committing code or configuration changes, run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
RUSTFLAGS="-D warnings" cargo dylint --all -- --all-targets --all-features --locked
cargo test --locked
```

Documentation-only changes do not require Rust checks. Add focused tests for changed behavior, using existing unit, UI, or consumer integration tests. Preserve source locations, scoped suppression, vocabulary collection, and Cargo invalidation when changing parsing or input discovery.

## Spelling and resources

Fix genuine typos; add legitimate technical terms to the case-sensitive `.spellcheck.txt`, not the upstream dictionary. Follow the README's [candidate workflow](README.md#generate-project-vocabulary) when regenerating vocabulary. Do not commit `.spellcheck.tmp`, its lock file, or coverage reports.

Dictionary resolution must remain offline. Do not execute referenced crate code or silently fall back when configured resources are broken. Preserve dictionary licenses and provenance.

## Documentation and releases

Update the README for user-facing changes. Keep Markdown paragraphs on single source lines. Add behavior changes under `[Unreleased]` in `CHANGELOG.md`, with the PR link and contributor as described in [CONTRIBUTING.md](CONTRIBUTING.md#documentation-and-changelog).

Follow [RELEASING.md](RELEASING.md) when asked to release. The library is distributed through Git; crates.io publishing is disabled. Never rewrite published release tags.
