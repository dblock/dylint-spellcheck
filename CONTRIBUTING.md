# Contributing

Contributions are welcome through [issues](https://github.com/dblock/dylint-spellcheck/issues) and [pull requests](https://github.com/dblock/dylint-spellcheck/pulls).

## Development setup

Install Rust through [rustup](https://rustup.rs/) and your platform's native linker and build tools (Xcode command-line tools on macOS, or a C/C++ build toolchain on Linux). Clone the repository and install Dylint:

```sh
git clone https://github.com/dblock/dylint-spellcheck.git
cd dylint-spellcheck
rustup show
cargo +stable install cargo-dylint dylint-link --version 6.2.0 --locked
```

Rustup installs the nightly compiler and components pinned in `rust-toolchain.toml`. Run development commands from the repository root without `+stable` so they use that pinned toolchain; stable is used above only to install the Dylint tools.

## Checks

Run these before submitting a change:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

CI runs these checks on Linux and macOS. Add focused regressions for changed behavior and review expected compiler diagnostics when updating UI fixtures.

Tests cover parsing and dictionary behavior, compiler diagnostics (`ui/`), and complete consumer projects (`tests/integration/`). Consumer tests share one built lint library but use isolated target directories so edits, vocabulary promotion, and workspace ownership exercise real Cargo invalidation. Readable cross-format fixtures live in `tests/fixtures/project/`.

### Dylint

A separate Linux workflow runs the pinned upstream general-purpose lint collection and this checkout's spellchecker against all Rust targets:

```sh
RUSTFLAGS="-D warnings" cargo dylint --all -- --all-targets --all-features --locked
```

The metadata uses `path = "."` to check the current implementation. Fix typos and add legitimate terms to the case-sensitive `.spellcheck.txt`. For regeneration, follow the README's [candidate workflow](README.md#generate-project-vocabulary), omitting `RUSTFLAGS="-D warnings"` while collecting. UI fixtures intentionally contain errors and run separately through diagnostic tests.

Filesystem tests use isolated temporary directories. Narrow allowances on those tests suppress the general collection's `non_thread_safe_call_in_test` false positives without disabling the rule elsewhere.

### Coverage

CI combines Linux and macOS reports on [Coveralls](https://coveralls.io/github/dblock/dylint-spellcheck). The repository must be enabled in Coveralls; uploads use the GitHub Actions token.

The pinned toolchain includes `llvm-tools-preview`. To generate the same report locally:

```sh
cargo +stable install cargo-llvm-cov --locked
cargo llvm-cov --all-features --workspace --locked --fail-under-lines 95 --lcov --output-path lcov.info
```

This runs the tests and enforces 95% line coverage; a separate `cargo test` is unnecessary. Consumer tests build an instrumented lint library in the coverage target directory, so compiler-subprocess execution contributes to the report. To inspect remaining gaps, run `cargo llvm-cov report --show-missing-lines`.

## Documentation and changelog

Update the README when user-facing behavior or configuration changes. Keep Markdown prose paragraphs on single source lines. Add changelog entries under `[Unreleased]` with a linked PR number and contributor, using `[#123](https://github.com/dblock/dylint-spellcheck/pull/123): Description - [@contributor](https://github.com/contributor).` Fill in the assigned PR number after opening the PR.

See [RELEASING.md](RELEASING.md) for releases.
