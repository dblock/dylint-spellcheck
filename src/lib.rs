#![feature(rustc_private)]

extern crate rustc_errors;
extern crate rustc_lexer;
extern crate rustc_span;

use dylint_linting::config;
use regex::Regex;
use rustc_errors::DiagDecorator;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, SyntaxContext};
use std::{path::PathBuf, sync::LazyLock};

mod spelling;
use spelling::{Checker, Config};

dylint_linting::declare_late_lint! {
    /// Checks words in Rust line and block comments using a configured Hunspell dictionary.
    /// URLs, email addresses and backtick-delimited code are skipped.
    /// This source-scanning lint currently supports crate-level suppression only.
    pub SPELLCHECK,
    Warn,
    "misspelled words in comments"
}

static SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)```.*?```|`[^`]*`|https?://[^\s<>]+|www\.[^\s<>]+|[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}",
    )
    .unwrap()
});
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}+(?:['’]\p{L}+)*").unwrap());

fn misspellings<'a>(text: &'a str, checker: &Checker) -> Vec<(usize, &'a str)> {
    let skipped: Vec<_> = SKIP.find_iter(text).map(|m| m.range()).collect();
    WORD.find_iter(text)
        .filter(|word| !skipped.iter().any(|range| range.contains(&word.start())))
        .filter(|word| !checker.correct(word.as_str()))
        .map(|word| (word.start(), word.as_str()))
        .collect()
}

impl<'tcx> LateLintPass<'tcx> for Spellcheck {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        let settings = match config::<Config>("dylint-spellcheck") {
            Ok(Some(settings)) => settings,
            Ok(None) => Config::default(),
            Err(error) => {
                cx.sess()
                    .dcx()
                    .err(format!("invalid dylint-spellcheck configuration: {error}"));
                return;
            }
        };
        let root = match configuration_root(cx) {
            Ok(root) => root,
            Err(error) => {
                cx.sess().dcx().err(error);
                return;
            }
        };
        let checker = match Checker::load(settings, &root) {
            Ok(checker) => checker,
            Err(error) => {
                cx.sess().dcx().err(error);
                return;
            }
        };
        for path in &checker.files {
            if let Err(error) = cx.sess().source_map().load_file(path) {
                cx.sess().dcx().err(format!(
                    "could not track dictionary input {}: {error}",
                    path.display()
                ));
                return;
            }
        }
        for file in cx.sess().source_map().files().iter() {
            if !matches!(&file.name, rustc_span::FileName::Real(name) if name.local_path().is_some_and(|path| path.extension().is_some_and(|extension| extension == "rs")))
            {
                continue;
            }
            let Some(source) = file.src.as_ref() else {
                continue;
            };
            let mut end = 0;
            for token in tokenize(source, FrontmatterAllowed::Yes) {
                let start = end;
                end += token.len as usize;
                let text_end = match token.kind {
                    TokenKind::LineComment { .. } => end,
                    TokenKind::BlockComment {
                        terminated: true, ..
                    } => end - 2,
                    _ => continue,
                };
                let text_start = start + 2;
                for (offset, word) in misspellings(&source[text_start..text_end], &checker) {
                    let lo = file.start_pos + BytePos((text_start + offset).try_into().unwrap());
                    let hi = lo + BytePos(word.len().try_into().unwrap());
                    let span = Span::new(lo, hi, SyntaxContext::root(), None);
                    cx.emit_span_lint(
                        SPELLCHECK,
                        span,
                        DiagDecorator(|diag| {
                            diag.primary_message(format!("misspelled word: `{word}`"));
                            let suggestions = checker.suggestions(word);
                            if !suggestions.is_empty() {
                                diag.help(format!(
                                    "did you mean {}?",
                                    suggestions
                                        .iter()
                                        .map(|s| format!("`{s}`"))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ));
                            }
                        }),
                    );
                }
            }
        }
    }
}

fn configuration_root(cx: &LateContext<'_>) -> Result<PathBuf, String> {
    let source = cx
        .sess()
        .local_crate_source_file()
        .and_then(rustc_span::RealFileName::into_local_path);
    let directory = source
        .as_deref()
        .and_then(std::path::Path::parent)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    cargo_metadata::MetadataCommand::new()
        .current_dir(directory)
        .no_deps()
        .exec()
        .map(|metadata| metadata.workspace_root.into_std_path_buf())
        .map_err(|error| format!("could not resolve spelling configuration directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Checker {
        Checker::load(
            Config {
                aff: Some("tests/fixtures/en.aff".into()),
                dic: Some("tests/fixtures/en.dic".into()),
                ..Config::default()
            },
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        )
        .unwrap()
    }

    #[test]
    fn checks_words_with_byte_offsets_and_ignores_addresses_and_code() {
        let text = "hello café helo `typpo` https://typpo.test a@typpo.test";
        assert_eq!(misspellings(text, &checker()), vec![(12, "helo")]);
        let mut checker = checker();
        checker.ignored_words.insert("helo".into());
        assert!(misspellings("helo", &checker).is_empty());
    }

    #[test]
    fn ui() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let settings = format!(
            "[dylint-spellcheck]\naff = {:?}\ndic = {:?}\nignored_words = [\"Kotoshu\"]\n",
            root.join("tests/fixtures/en.aff"),
            root.join("tests/fixtures/en.dic"),
        );
        dylint_testing::ui::Test::src_base("dylint_spellcheck", "ui")
            .dylint_toml(&settings)
            .run();
    }
}
