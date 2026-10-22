# Changelog

## [Unreleased]

### Added

- [#26](https://github.com/dblock/dylint-spellcheck/pull/26): Safely parse YAML front matter with recursive string checks, dotted field paths under `markdown.front_matter`, suppression, and precise escaped source spans - [@dblock](https://github.com/dblock).
- [#23](https://github.com/dblock/dylint-spellcheck/pull/23): Check Rustdoc string attributes and included documentation with scoped suppression, source mapping, and duplicate diagnostic prevention - [@dblock](https://github.com/dblock).
- [#11](https://github.com/dblock/dylint-spellcheck/pull/11): Check selected plain-text documents literally, automatically excluding configured dictionary and vocabulary resources - [@dblock](https://github.com/dblock).
- [#10](https://github.com/dblock/dylint-spellcheck/pull/10): Respect scoped Rust lint attributes and Markdown suppression directives, excluding suppressed words from vocabulary candidates - [@dblock](https://github.com/dblock).
- [#9](https://github.com/dblock/dylint-spellcheck/pull/9): Configure dictionaries and vocabulary with optional language keys, directory discovery, and offline `crate:name/path` resources that override default English - [@dblock](https://github.com/dblock).
- [#5](https://github.com/dblock/dylint-spellcheck/pull/5): Select standalone files with include/exclude patterns and dispatch Markdown inputs with discovery dependency tracking and workspace-target ownership - [@dblock](https://github.com/dblock).
- [#4](https://github.com/dblock/dylint-spellcheck/pull/4): Parse Markdown prose in Rust comments, skipping code and link destinations while preserving source locations - [@dblock](https://github.com/dblock).
- [#2](https://github.com/dblock/dylint-spellcheck/pull/2): Collect spelling candidates in a coordinated temporary file when a declared vocabulary is missing, without suggestions or source edits - [@dblock](https://github.com/dblock).
- [#1](https://github.com/dblock/dylint-spellcheck/pull/1): Bundle English dictionaries and support configuration-relative dictionaries and project vocabulary files - [@dblock](https://github.com/dblock).
