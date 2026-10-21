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

CI runs the same checks on Linux and macOS. Tests cover both bundled English and custom dictionary fixtures. UI tests cover real compiler diagnostics, comment extraction, source positions, and crate-level suppression. Add focused regressions for changed behavior and review expected diagnostic output when updating UI fixtures.

### Dylint

A separate Linux workflow runs the pinned upstream general-purpose lint collection and this checkout's spellchecker against all Rust targets:

```sh
RUSTFLAGS="-D warnings" cargo dylint --all -- --all-targets --all-features --locked
```

The metadata uses `path = "."`, so spelling checks exercise the current implementation rather than a previously published revision. Fix genuine typos and put accepted technical terms in the case-sensitive `.spellcheck.txt` vocabulary configured by `dylint.toml`. To regenerate candidates, move the vocabulary aside and run the same command without `RUSTFLAGS="-D warnings"`, review `.spellcheck.tmp`, and rename it to `.spellcheck.txt`. UI fixtures intentionally contain spelling errors and are checked separately by the diagnostic tests, not compiled as Cargo targets.

Filesystem tests use isolated temporary directories. Narrow allowances on those tests suppress the general collection's `non_thread_safe_call_in_test` false positives without disabling the rule elsewhere.

## Documentation and changelog

Update the README when user-facing behavior or configuration changes. Keep Markdown prose paragraphs on single source lines. Add changelog entries under `[Unreleased]` with a linked PR number and contributor, using `[#123](https://github.com/dblock/dylint-spellcheck/pull/123): Description - [@contributor](https://github.com/contributor).` Fill in the assigned PR number after opening the PR.
