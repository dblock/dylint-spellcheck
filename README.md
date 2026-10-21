# dylint-spellcheck

A minimal [Dylint](https://github.com/trailofbits/dylint) spelling lint powered by [kotoshu-rs](https://github.com/kotoshu/kotoshu-rs).

Checks Rust line comments, documentation comments, and block comments using bundled English, reporting misspelled words with source locations and up to three suggestions. Comments are parsed as Markdown: prose, headings, link labels, and image descriptions are checked; inline code, fenced and indented code blocks, link destinations, HTML, URLs, and email addresses are skipped. Adjacent line comments of the same kind are combined, so Rustdoc code fences can span lines. Strings and identifiers are not checked.

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
```

Vocabulary files contain one accepted word per line, for example `Kotoshu`. Blank lines and lines beginning with `#` are ignored. Vocabulary is case-sensitive and supplements rather than replaces the dictionary.

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

Relative paths resolve from the workspace root containing `dylint.toml`, including when compiling workspace members from another working directory. Absolute paths are supported. A missing vocabulary file triggers candidate collection as described below; unreadable files, missing parent directories, an incomplete `aff`/`dic` pair, or invalid configuration fail explicitly. Dictionary and vocabulary edits invalidate compiler checks. No resources are downloaded at runtime.

For reproducible builds, pin the library's Git dependency to a reviewed commit with `rev`. Dylint installs the library's pinned nightly toolchain; this does not change your project's normal toolchain.

## Limitations

Markdown parsing follows CommonMark with tables, strikethrough, task lists, and footnotes. HTML markup and HTML blocks are skipped; Markdown inside HTML blocks is not checked. Comments separated by Rust code, a blank source line, or a different comment kind are separate documents. Rustdoc attributes such as `#[doc = "..."]` are not checked. Technical tokens may need project vocabulary entries. No autocorrection is performed. Suppress the lint at the crate level with `#![allow(spellcheck)]`; item-level suppression is not supported by this source-scanning implementation.

## Standalone Markdown

Select additional files independently of their parser in the workspace-root `dylint.toml`:

```toml
[dylint-spellcheck]
include = ["README.md", "docs/**/*.md", "docs/**/*.markdown"]
exclude = ["docs/generated/**"]
vocabulary_files = [".spellcheck.txt"]
```

Paths and patterns resolve from the workspace root; absolute paths are also supported. `include` accepts exact file paths and glob patterns (`*`, `?`, character classes, and recursive `**`). `exclude` takes precedence over inclusion. Globs may match no files; missing exact files, directories selected as exact files, unreadable inputs, and malformed patterns fail explicitly. Repeated paths to the same file are checked only once. Directory symlinks are not followed during discovery.

Selected files are dispatched by format, rather than assuming all included files are Markdown. Currently `.md` and `.markdown` (case-insensitive) use the Markdown parser; other selected formats fail explicitly until a parser is supported. Compiled Rust comments remain checked by default; `include` and `exclude` select additional standalone inputs, not compiler source files. Documents share dictionaries, vocabulary, suggestions, and candidate collection with Rust comments, with diagnostics pointing directly into the document.

Document edits and additions or removals under glob search directories invalidate Cargo's checks; no Rust source edit or clean build is needed. Directory tracking is conservative: unrelated directory-entry changes may also trigger rechecking. Prefer narrow patterns such as `docs/**/*.md` and exclude generated directories to limit discovery and rebuilds.

Run the complete workspace scope:

```sh
cargo dylint --all -- --workspace --all-targets --all-features
```

To avoid repeating workspace documents for every binary and test, one compilation target owns them: the workspace-root package, or the member with the lexicographically first manifest path in a virtual workspace. Its library (including a procedural macro) is preferred; otherwise its first binary by name without required features is preferred. Test-harness variants do not check documents. The owning target must be selected for document checks to run; selecting only other packages, binaries, or test harnesses does not check standalone documents. Multiple target triples are separate compilations and may each report the documents. Crate-level suppression on the owning target also suppresses document diagnostics and vocabulary candidates.

## Generate project vocabulary

Declare a vocabulary file that does not exist yet:

```toml
[dylint-spellcheck]
vocabulary_files = [".spellcheck.txt"]
```

Run Dylint normally, selecting the full intended workspace and feature scope:

```sh
cargo dylint --all -- --workspace --all-targets --all-features
```

```text
note: Collected spelling candidates in .spellcheck.tmp. Review for genuine typos before renaming to .spellcheck.txt. This list accumulates across runs; delete it before a fresh collection.
```

While the declared vocabulary is missing, spelling diagnostics remain active but suggestions are not computed. Unknown words are accumulated, sorted, and deduplicated in the sibling `.tmp` file, preserving capitalization. Concurrent compiler processes coordinate writes using a sibling `.tmp.lock` file. Neither the temporary vocabulary nor its lock file needs to be committed. No source or configuration files are changed, and the declared vocabulary is never created automatically.

**Review `.spellcheck.tmp`: fix genuine typos in source and remove them from the candidate list.** Then promote it:

```sh
mv .spellcheck.tmp .spellcheck.txt
```

Once the declared vocabulary exists, it is only read, normal suggestions resume, and candidate collection stops. Its appearance is tracked so Cargo rechecks affected targets after promotion. Other existing vocabulary files and explicit ignored words are honored during collection, as is crate-level spelling suppression.

The temporary file is an accumulating candidate list, not an exact snapshot: words from older runs remain until you remove them. To regenerate, remove or move the accepted vocabulary and delete its `.tmp` file, then rerun Dylint. Missing vocabulary paths must end in `.txt` and their parent directories must exist. For several missing vocabulary files, each receives the same candidate list. Compilation failures can leave partial candidates, and unselected targets or disabled features are not checked. Warnings promoted to errors still fail the build, but candidates from completed lint passes are written for review.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and checks.
