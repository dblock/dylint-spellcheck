# dylint-spellcheck

A minimal [Dylint](https://github.com/trailofbits/dylint) spelling lint powered by [kotoshu-rs](https://github.com/kotoshu/kotoshu-rs).

Checks Rust line comments, documentation comments, and block comments against a Hunspell `.aff`/`.dic` dictionary, reporting misspelled words with source locations and up to three suggestions. Strings and identifiers are not checked. URLs, email addresses, and backtick-delimited code within a comment are skipped.

## Setup

Install Rust through [rustup](https://rustup.rs/), then install Dylint:

```sh
cargo +stable install cargo-dylint dylint-link --version 6.2.0 --locked
```

Add this library to your project's `Cargo.toml`:

```toml
[workspace.metadata.dylint]
libraries = [
    { git = "https://github.com/dblock/dylint-spellcheck" },
]
```

Create `dylint.toml` in your workspace root, pointing to an existing Hunspell dictionary:

```toml
[dylint-spellcheck]
aff = "/absolute/path/to/en_US.aff"
dic = "/absolute/path/to/en_US.dic"
ignored_words = ["Kotoshu"]
```

Dictionary paths are resolved from the compiler's working directory; absolute paths avoid ambiguity across workspace members. Missing configuration or unreadable dictionaries fail explicitly. Dictionaries are not bundled or downloaded. Ignored words are case-sensitive.

```sh
cargo dylint --all
```

For reproducible builds, pin the library's Git dependency to a reviewed commit with `rev`. Dylint installs the library's pinned nightly toolchain; this does not change your project's normal toolchain.

## Limitations

This first version scans comments as plain text, not Markdown prose. Backtick spans can only be skipped within one comment token; fenced blocks spanning several `//` lines are not recognized. Technical tokens may need `ignored_words`. No autocorrection is performed. Suppress the lint at the crate level with `#![allow(spellcheck)]`; item-level suppression is not supported by this source-scanning implementation.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

Tests use a tiny dictionary fixture, not a general-purpose English dictionary. UI tests cover real compiler diagnostics, comment extraction, source positions, and crate-level suppression.
