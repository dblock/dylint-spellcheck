# dylint-spellcheck

A minimal [Dylint](https://github.com/trailofbits/dylint) spelling lint powered by [kotoshu-rs](https://github.com/kotoshu/kotoshu-rs).

Checks Rust line comments, documentation comments, and block comments using bundled English, reporting misspelled words with source locations and up to three suggestions. Strings and identifiers are not checked. URLs, email addresses, and backtick-delimited code within a comment are skipped.

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

Run the spelling checks; no dictionary configuration is required:

```sh
cargo dylint --all
```

## Usage

For example, put this in `src/main.rs`:

```rust
// hello helo
fn main() {}
```

Running `cargo dylint --all` reports:

```text
warning: misspelled word: `helo`
 --> src/main.rs:1:10
  |
1 | // hello helo
  |          ^^^^
  |
  = help: did you mean `hello`, `help`, `hero`?
  = note: `#[warn(spellcheck)]` on by default
```

## Dictionaries and project vocabulary

The default is the bundled [American English Hunspell dictionary](data/en/README.md), with its full license and provenance. Add a `dylint.toml` file at your workspace root to supplement English with a project vocabulary:

```toml
[dylint-spellcheck]
vocabulary_files = [".spellcheck.txt"]
ignored_words = ["Kotoshu"]
```

Vocabulary files contain one accepted word per line. Blank lines and lines beginning with `#` are ignored. Vocabulary and `ignored_words` are case-sensitive; neither replaces the dictionary.

### Use another language

Download a Hunspell dictionary and its license from [Kotoshu's dictionaries](https://github.com/kotoshu/dictionaries). For example, download the [latest German resources](https://github.com/kotoshu/dictionaries/tree/v1/de/spelling):

```sh
mkdir -p dictionaries/de
curl -fL https://raw.githubusercontent.com/kotoshu/dictionaries/v1/de/spelling/index.dic -o dictionaries/de/de.dic
curl -fL https://raw.githubusercontent.com/kotoshu/dictionaries/v1/de/spelling/index.aff -o dictionaries/de/de.aff
curl -fL https://raw.githubusercontent.com/kotoshu/dictionaries/v1/de/spelling/license -o dictionaries/de/LICENSE
```

The matching `.aff` file is required. Commit both files and their license, or provision them in CI. Review each dictionary's license before redistributing it; the German resources are licensed under [GPL-2.0 OR GPL-3.0](https://github.com/kotoshu/dictionaries/blob/v1/de/spelling/license) and are not bundled. For reproducible downloads, replace `v1` with a reviewed dictionary repository commit.

Configure both paths in the workspace-root `dylint.toml`:

```toml
[dylint-spellcheck]
aff = "dictionaries/de/de.aff"
dic = "dictionaries/de/de.dic"
vocabulary_files = [".spellcheck.txt"]
```

Create `.spellcheck.txt` with any project-specific words, or omit `vocabulary_files` if you do not need a supplement. Run `cargo dylint --all` as usual. No language code or additional Rust dependency is required: the configured dictionary determines which words are accepted.

An explicit `aff`/`dic` pair **replaces bundled English**, rather than adding another language alongside it. Only one Hunspell dictionary can be active at a time; simultaneous English and German dictionaries are not currently supported. Vocabulary files can supplement that dictionary with individual words, but do not provide another language's affix rules.

Relative paths resolve from the workspace root containing `dylint.toml`, including when compiling workspace members from another working directory. Absolute paths are supported. A missing vocabulary file, unreadable dictionary, incomplete `aff`/`dic` pair, or invalid configuration fails explicitly. Dictionary and vocabulary edits invalidate compiler checks. No resources are downloaded at runtime.

For reproducible builds, pin the library's Git dependency to a reviewed commit with `rev`. Dylint installs the library's pinned nightly toolchain; this does not change your project's normal toolchain.

## Limitations

This first version scans comments as plain text, not Markdown prose. Backtick spans can only be skipped within one comment token; fenced blocks spanning several `//` lines are not recognized. Technical tokens may need `ignored_words`. No autocorrection is performed. Suppress the lint at the crate level with `#![allow(spellcheck)]`; item-level suppression is not supported by this source-scanning implementation.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and checks.
