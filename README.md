# dylint-spellcheck

[![Test](https://github.com/dblock/dylint-spellcheck/actions/workflows/test.yml/badge.svg)](https://github.com/dblock/dylint-spellcheck/actions/workflows/test.yml)
[![Lint](https://github.com/dblock/dylint-spellcheck/actions/workflows/lint.yml/badge.svg)](https://github.com/dblock/dylint-spellcheck/actions/workflows/lint.yml)
[![Coverage](https://coveralls.io/repos/github/dblock/dylint-spellcheck/badge.svg?branch=main)](https://coveralls.io/github/dblock/dylint-spellcheck?branch=main)

A [Dylint](https://github.com/trailofbits/dylint) spelling lint for Rust comments, Markdown, and plain text, powered by [kotoshu-rs](https://github.com/kotoshu/kotoshu-rs).

Checks Rust comments and Rustdoc attributes using bundled English and reports misspelled words with source locations and up to three suggestions. Add Markdown and plain-text documents with `include`. Ordinary strings and identifiers are not checked.

- [Setup](#setup)
- [Usage](#usage)
- [Rustdoc attributes](#rustdoc-attributes)
- [Standalone documents](#standalone-documents)
- [YAML front matter](#yaml-front-matter)
- [Scoped suppression](#scoped-suppression)
- [Generate project vocabulary](#generate-project-vocabulary)
- [Dictionaries and languages](#dictionaries-and-languages)
- [Word boundaries](#word-boundaries)
- [Contributing](#contributing)

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

For reproducible builds, add `rev = "<reviewed commit>"` to the library entry. Dylint installs the library's pinned nightly compiler without changing your project's normal toolchain.

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

To collect project-specific words, add `dylint.toml` at your workspace root:

```toml
[dylint-spellcheck]
dictionaries = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
```

Run `cargo dylint --all` again. If `.spellcheck.txt` does not exist, the lint collects words not recognized by the English dictionary into `.spellcheck.tmp`. Review the candidates, fix genuine typos such as `helo` in source, and remove those typos from the list. Rename the reviewed list to accept the remaining project-specific words:

```sh
mv .spellcheck.tmp .spellcheck.txt
```

Commit `.spellcheck.txt` with `dylint.toml`. Subsequent runs read the vocabulary without modifying it or collecting new candidates.

## Rustdoc attributes

Documentation in `///`, `//!`, `#[doc = "..."]`, raw strings, and `#![doc = include_str!("../README.md")]` is checked as Markdown. Adjacent attributes and documentation comments on an item are combined in order. Diagnostics point into the literal or included UTF-8 file; other compiler-expanded expressions use their call-site span when exact mapping is unavailable.

Rust lint attributes on the documented item control these diagnostics and vocabulary candidates. Rust's `include_str!` dependency tracking triggers rechecks when included files change. Included Rustdoc takes precedence over standalone selection in that compilation, and repeated diagnostics at the same source location are emitted once. Separate compilation targets can still report the same included document.

## Standalone documents

Add documents to the workspace-root `dylint.toml`:

```toml
[dylint-spellcheck]
include = ["README.md", "docs/**/*.md", "docs/**/*.markdown", "docs/**/*.txt"]
exclude = ["docs/generated/**"]
dictionaries = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
```

Rust comments and `.md`/`.markdown` documents are parsed as Markdown. Prose, headings, link labels, and image descriptions are checked; inline code, fenced and indented code, link destinations, and HTML are skipped. Adjacent Rust line comments of the same kind are combined, so Rustdoc fences can span lines. Plain `.txt` documents are checked literally, including backticks and markup. Extensions are case-insensitive; URLs and email addresses are skipped in both formats.

Paths resolve from the workspace root; absolute paths are supported. `include` accepts exact files and globs (`*`, `?`, character classes, and `**`); `exclude` takes precedence. Globs may match nothing, but missing exact files, unreadable inputs, malformed patterns, and unsupported formats are errors. Files are deduplicated, configured dictionary/vocabulary resources are excluded, and directory symlinks are not followed. These settings select additional documents, not Rust source files.

Run the full workspace:

```sh
cargo dylint --all -- --workspace --all-targets --all-features
```

Document edits, additions, and removals trigger Cargo rechecks without Rust source changes. Prefer narrow globs to avoid unnecessary directory scanning and rechecks.

Liquid expressions (`{{ ... }}`) and tags (`{% ... %}`) are skipped in Markdown and Rust documentation, including multiline forms. Each ends at its first matching closing delimiter; unmatched openers remain literal. Only the delimited expression or tag is excluded, not surrounding prose. Templates are never evaluated or loaded, and directives inside them cannot change spelling suppression. Plain text remains literal.

### Workspace ownership

One compilation target checks documents to avoid duplicates: the workspace-root package, or the member with the lexicographically first manifest path in a virtual workspace. Its library is preferred; otherwise its first binary by name without required features is preferred. Test-harness variants do not check documents. Select the owning target for document checks to run. Separate target triples may each report documents; crate-level suppression on the owner also suppresses document diagnostics and vocabulary candidates.

## YAML front matter

Markdown documents, Rust comments, and Rustdoc attributes recognize YAML front matter at the start of a document or combined comment group, between opening `---` and closing `---` or `...` lines. UTF-8 BOMs and CRLF line endings are supported. The body is checked normally.

String values are checked recursively, including quoted and multiline strings; keys, YAML comments, booleans, nulls, and numbers are skipped under the YAML 1.2 core schema. Use quotes or `!!str` to check a non-string value as prose. Anchors are checked once; aliases and custom-tagged values are skipped. Parsing never constructs objects or loads external resources.

Configure front matter under the Markdown settings, independently of document file selection:

```toml
[dylint-spellcheck.markdown.front_matter]
enabled = true
include = ["title", "description", "authors.name"]
exclude = ["description.internal"]
```

`enabled` defaults to `true`; set it to `false` to skip metadata spelling. Omitted `include` checks all strings, `include = []` checks none, and `exclude` always wins. Case-sensitive dotted paths select a field and its descendants; `authors.name` selects `name` in every author mapping, without array indices. Paths have no wildcards or escaping; unknown paths match nothing, and empty segments are errors.

Each string uses Markdown and suppression rules independently of other strings and the body. Quoting, escapes, Unicode, folding, and entities retain source locations. Invalid YAML or a missing closing delimiter is an error even when metadata spelling is disabled.

## Scoped suppression

Use `#[allow(spellcheck)]` on Rust items and blocks, or `#![allow(spellcheck)]` for a crate or module. Documentation comments belong to their item; ordinary comments use their enclosing scope. Ordinary comments preceding an item are outside that item's scope. Nested `#[warn(spellcheck)]` and `#[deny(spellcheck)]` follow Rust's normal lint-level rules.

For Markdown documents and Rust comments, use HTML-comment directives:

```markdown
<!-- spellcheck:disable -->
Intentional misspellings.
<!-- spellcheck:enable -->
Checked prose.
Intentional example. <!-- spellcheck:todo -->
```

Standalone `disable`/`enable` directives control subsequent lines. A trailing `disable` or any `todo` suppresses only its own line. Add a reason with ` -- ` inside the comment. Directives inside code are ignored, and state is local to each combined Rust comment group. Plain text does not interpret these directives.

Suppressed words produce neither diagnostics nor vocabulary candidates. Standalone documents use their owning target's crate-level lint setting, not individual Rust item attributes.

## Generate project vocabulary

Declare a missing `.txt` vocabulary as shown in [Usage](#usage). Its parent directory must exist. Select the full intended workspace and feature scope:

```sh
cargo dylint --all -- --workspace --all-targets --all-features
```

While the vocabulary is missing, diagnostics remain active but suggestions are skipped. Unknown words accumulate in a sorted, deduplicated `.tmp` file, preserving capitalization. Concurrent compilers coordinate through `.tmp.lock`; ignore both temporary files in Git. Source, configuration, and accepted vocabulary are never rewritten.

**Fix typos in source and remove them from `.spellcheck.tmp` before renaming it to `.spellcheck.txt`.** Promotion triggers a recheck, restores suggestions, and stops collection. Other dictionaries and scoped suppression remain active.

To regenerate, move the accepted vocabulary aside, delete its `.tmp`, and rerun. Candidates accumulate rather than reflecting an exact snapshot; multiple missing vocabulary files receive the same list. Failed compilations can leave partial results, and omitted targets/features are not checked. Warnings promoted to errors still fail the build but retain candidates from completed lint passes.

## Dictionaries and languages

The default is bundled [American English](data/en/README.md). Configure `dictionaries` in the workspace-root `dylint.toml` to add vocabulary or replace it.

Entries accept `.txt` vocabulary files, dictionary directories, and `crate:<name>/<path>` references. Vocabulary is case-sensitive, with one accepted word per line; blank lines and `#` comments are ignored. All listed resources are cumulative: any dictionary or vocabulary can accept a word. Suggestions are deduplicated, with up to three returned.

Language names are optional. Without a language, each directory loads all matching `.aff`/`.dic` pairs directly inside it (not recursively). To select a particular pair by convention, use a language table instead of the list:

```toml
[dylint-spellcheck.dictionaries]
en = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
de = ["dictionaries/de", ".spellcheck.de.txt"]
```

The `de` entry loads `dictionaries/de/de.aff`, `dictionaries/de/de.dic`, and `.spellcheck.de.txt`. Language keys only select filenames; they never load dictionaries implicitly. Use either a list or a language table, not both.

Omitting `dictionaries` defaults to `["crate:dylint-spellcheck/data/en"]`. An explicit list or language table replaces that default; English is loaded only if referenced. An empty list or table loads no dictionaries. To override English, use `dictionaries = ["dictionaries/en", ".spellcheck.txt"]` without the crate reference.

### Crate resources

`crate:<name>/<path>` reads a directory or existing `.txt` file inside a locally resolved Cargo package, for example `crate:my-dictionaries/resources/de` or `crate:my-dictionaries/words.txt`. Use the package name, not a dependency alias; no fixed resource layout is required.

Resolution uses offline, locked Cargo metadata for the lint's host platform. Packages and their host-platform dependencies must already be available, with an up-to-date `Cargo.lock`. Nothing is downloaded or executed from referenced crates. Missing or ambiguous packages, missing resources, and paths escaping the package are errors.

`crate:dylint-spellcheck/data/en` uses a locally resolved package's resources in preference to embedded English. If no such package is resolved, it uses embedded English without requiring an application dependency. This fallback supports only `data/en` and an optional `en` key; broken local resources never trigger fallback.

### Use another language

Download a Hunspell dictionary and its license from [Kotoshu's dictionaries](https://github.com/kotoshu/dictionaries). For example, download the [latest German resources](https://github.com/kotoshu/dictionaries/tree/v1/de/spelling):

```sh
mkdir -p dictionaries/de
curl -fL https://raw.githubusercontent.com/kotoshu/dictionaries/v1/de/spelling/index.dic -o dictionaries/de/de.dic
curl -fL https://raw.githubusercontent.com/kotoshu/dictionaries/v1/de/spelling/index.aff -o dictionaries/de/de.aff
curl -fL https://raw.githubusercontent.com/kotoshu/dictionaries/v1/de/spelling/license -o dictionaries/de/LICENSE
```

The matching `.aff` file is required. Commit both files and their license, or provision them in CI. Review each dictionary's license before redistributing it; the German resources are licensed under [GPL-2.0 OR GPL-3.0](https://github.com/kotoshu/dictionaries/blob/v1/de/spelling/license) and are not bundled. For reproducible downloads, replace `v1` with a reviewed dictionary repository commit.

Configure the directory in the workspace-root `dylint.toml`:

```toml
[dylint-spellcheck.dictionaries]
de = ["dictionaries/de", ".spellcheck.de.txt"]
```

Create `.spellcheck.de.txt` with project-specific words, or let the next run generate reviewable candidates. Omit it if no supplement is needed.

Alternatively, use `dictionaries = ["dictionaries/de", ".spellcheck.de.txt"]` under `[dylint-spellcheck]` to discover all dictionary pairs in that directory. This checks German without loading English. Add `"crate:dylint-spellcheck/data/en"` to the unqualified list, or an `en` table entry containing that reference, to check both languages.

Local paths resolve from the workspace root; absolute paths are supported. Missing `.txt` files trigger [candidate collection](#generate-project-vocabulary), but their parent directories must exist. Unreadable inputs, missing or incomplete dictionary pairs, empty dictionary directories, and invalid configuration are errors. Dictionary edits and pair additions invalidate compiler checks.

## Word boundaries

All formats and languages share Unicode-aware word boundaries. Letters and combining marks form words; internal straight and curly apostrophes are retained (`can't`, `can’t`). Curly apostrophes also try straight-apostrophe dictionary lookup. Diagnostics preserve the original spelling and source range. Other Unicode normalization and language-specific segmentation are not performed.

Tokens containing numbers or connector punctuation are skipped entirely: `1st`, `sha256`, `v2beta`, and `wrong_token` are not split into misleading fragments. Hyphens separate words, so `helo-world` reports `helo`; alphabetic hashes are checked as prose. Markdown formatting is decoded first: `_helo_` is checked as emphasis, but skipped as an underscore-containing token in plain text.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and checks, and [RELEASING.md](RELEASING.md) for the release procedure.
