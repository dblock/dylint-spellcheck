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

## Dictionaries and project vocabulary

The default is the bundled [American English Hunspell dictionary](data/en/README.md), with its full license and provenance. Add a `dylint.toml` file at your workspace root to supplement English with a project vocabulary:

```toml
[dylint-spellcheck]
dictionaries = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
```

The `dictionaries` list accepts `.txt` vocabulary files, dictionary directories, and `crate:<name>/<path>` resource references. Vocabulary files contain one accepted word per line, for example `Kotoshu`. Blank lines and lines beginning with `#` are ignored. Vocabulary is case-sensitive. All explicitly listed resources are cumulative: a word is accepted if any dictionary or vocabulary accepts it; suggestions are deduplicated across dictionaries, with up to three returned.

Language names are optional. Without a language, each directory loads all matching `.aff`/`.dic` pairs directly inside it (not recursively). To select a particular pair by convention, use a language table instead of the list:

```toml
[dylint-spellcheck.dictionaries]
en = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
de = ["dictionaries/de", ".spellcheck.de.txt"]
```

This loads `dictionaries/de/de.aff` and `dictionaries/de/de.dic`, plus both vocabulary files and the explicitly referenced packaged English dictionary. Multiple directories or vocabulary files per language are supported. Language keys only select filenames; they never implicitly load a dictionary. Use either the unqualified list or the language table, not both in the same configuration.

Omitting `dictionaries` defaults to `["crate:dylint-spellcheck/data/en"]`. An explicit list or language table replaces that default; English is loaded only if referenced. An empty list or table loads no dictionaries. To override English, use `dictionaries = ["dictionaries/en", ".spellcheck.txt"]` without the crate reference.

`crate:<name>/<path>` resolves the named package in the project's Cargo dependency graph using offline, locked Cargo metadata for the lint's host platform, then reads the specified package-relative directory or existing `.txt` file. Nothing is downloaded, installed, or executed from the referenced crate. The crate and its host-platform dependencies must already be locally available, and `Cargo.lock` must be up to date. Names must identify exactly one resolved package; unavailable or ambiguous packages, missing resources, and paths escaping the package fail explicitly. Dependency renames do not change the package name used in the reference.

For example, `crate:my-dictionaries/resources/de` reads dictionary pairs from that directory in the locally resolved `my-dictionaries` package; `crate:my-dictionaries/words.txt` reads its vocabulary file. No fixed data layout is required. A locally resolved `dylint-spellcheck` package takes precedence over this lint's embedded resources, so `crate:dylint-spellcheck/data/en` uses that package's files without also loading the embedded dictionary. Only when no such package is resolved does that reference use embedded English, without adding the lint as an application dependency. The embedded fallback supports only `data/en` and an optional `en` language key. Missing or broken resources in a resolved local package are errors, not grounds for falling back.

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

Create `.spellcheck.de.txt` with any project-specific words, or leave it out of the list if you do not need a supplement. Run `cargo dylint --all` as usual. No additional Rust dependency is required.

Alternatively, use `dictionaries = ["dictionaries/de", ".spellcheck.de.txt"]` under `[dylint-spellcheck]` to discover all dictionary pairs in that directory. This checks German without loading English. Add `"crate:dylint-spellcheck/data/en"` to the unqualified list, or an `en` table entry containing that reference, to check both languages.

Relative paths resolve from the workspace root containing `dylint.toml`, including when compiling workspace members from another working directory. Absolute paths are supported. A missing `.txt` vocabulary file triggers candidate collection as described below; unreadable files, missing parent directories, missing dictionary directories, incomplete `.aff`/`.dic` pairs, empty dictionary directories, or invalid configuration fail explicitly. Dictionary and vocabulary edits invalidate compiler checks, as do dictionary-pair additions in configured directories. No resources are downloaded at runtime.

For reproducible builds, pin the library's Git dependency to a reviewed commit with `rev`. Dylint installs the library's pinned nightly toolchain; this does not change your project's normal toolchain.

## Limitations

Markdown parsing follows CommonMark with tables, strikethrough, task lists, and footnotes. HTML markup and HTML blocks are skipped; Markdown inside HTML blocks is not checked. Comments separated by Rust code, a blank source line, or a different comment kind are separate documents. Rustdoc attributes such as `#[doc = "..."]` are not checked. Technical tokens may need project vocabulary entries. No autocorrection is performed. Suppress the lint at the crate level with `#![allow(spellcheck)]`; item-level suppression is not supported by this source-scanning implementation.

## Standalone Markdown

Select additional files independently of their parser in the workspace-root `dylint.toml`:

```toml
[dylint-spellcheck]
include = ["README.md", "docs/**/*.md", "docs/**/*.markdown"]
exclude = ["docs/generated/**"]
dictionaries = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
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
dictionaries = ["crate:dylint-spellcheck/data/en", ".spellcheck.txt"]
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

Once the declared vocabulary exists, it is only read, normal suggestions resume, and candidate collection stops. Its appearance is tracked so Cargo rechecks affected targets after promotion. All other dictionaries and vocabulary files are honored during collection, as is crate-level spelling suppression.

The temporary file is an accumulating candidate list, not an exact snapshot: words from older runs remain until you remove them. To regenerate, remove or move the accepted vocabulary and delete its `.tmp` file, then rerun Dylint. Missing vocabulary paths must end in `.txt` and their parent directories must exist. For several missing vocabulary files, each receives the same candidate list. Compilation failures can leave partial candidates, and unselected targets or disabled features are not checked. Warnings promoted to errors still fail the build, but candidates from completed lint passes are written for review.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and checks.
