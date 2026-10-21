#![feature(rustc_private)]

extern crate rustc_errors;
extern crate rustc_lexer;
extern crate rustc_span;

use dylint_linting::config;
use kotoshu::dict::Dictionary;
use regex::Regex;
use rustc_errors::DiagDecorator;
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, SyntaxContext};
use serde::Deserialize;
use std::{collections::HashSet, path::PathBuf, sync::LazyLock};

dylint_linting::declare_late_lint! {
    /// Checks words in Rust line and block comments using a configured Hunspell dictionary.
    /// URLs, email addresses and backtick-delimited code are skipped.
    /// This source-scanning lint currently supports crate-level suppression only.
    pub SPELLCHECK,
    Warn,
    "misspelled words in comments"
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    aff: PathBuf,
    dic: PathBuf,
    #[serde(default)]
    ignored_words: HashSet<String>,
}

static SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?s)```.*?```|`[^`]*`|https?://[^\s<>]+|www\.[^\s<>]+|[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}",
    )
    .unwrap()
});
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}+(?:['’]\p{L}+)*").unwrap());

fn misspellings<'a>(
    text: &'a str,
    dictionary: &Dictionary,
    ignored: &HashSet<String>,
) -> Vec<(usize, &'a str)> {
    let skipped: Vec<_> = SKIP.find_iter(text).map(|m| m.range()).collect();
    WORD.find_iter(text)
        .filter(|word| !skipped.iter().any(|range| range.contains(&word.start())))
        .filter(|word| !ignored.contains(word.as_str()) && !dictionary.correct(word.as_str()))
        .map(|word| (word.start(), word.as_str()))
        .collect()
}

impl<'tcx> LateLintPass<'tcx> for Spellcheck {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        let settings = match config::<Config>("dylint-spellcheck") {
            Ok(Some(settings)) => settings,
            Ok(None) => {
                cx.sess().dcx().err("dylint-spellcheck requires [dylint-spellcheck] with aff and dic paths in dylint.toml");
                return;
            }
            Err(error) => {
                cx.sess()
                    .dcx()
                    .err(format!("invalid dylint-spellcheck configuration: {error}"));
                return;
            }
        };
        let dictionary = match Dictionary::load(&settings.aff, &settings.dic) {
            Ok(dictionary) => dictionary,
            Err(error) => {
                cx.sess()
                    .dcx()
                    .err(format!("could not load spelling dictionary: {error}"));
                return;
            }
        };
        for file in cx.sess().source_map().files().iter() {
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
                for (offset, word) in misspellings(
                    &source[text_start..text_end],
                    &dictionary,
                    &settings.ignored_words,
                ) {
                    let lo = file.start_pos + BytePos((text_start + offset).try_into().unwrap());
                    let hi = lo + BytePos(word.len().try_into().unwrap());
                    let span = Span::new(lo, hi, SyntaxContext::root(), None);
                    cx.emit_span_lint(
                        SPELLCHECK,
                        span,
                        DiagDecorator(|diag| {
                            diag.primary_message(format!("misspelled word: `{word}`"));
                            let suggestions = dictionary.suggest(word, 3);
                            if !suggestions.is_empty() {
                                diag.help(format!(
                                    "did you mean {}?",
                                    suggestions
                                        .iter()
                                        .map(|s| format!("`{}`", s.word))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dictionary() -> Dictionary {
        Dictionary::load_from_sources(
            include_str!("../tests/fixtures/en.aff"),
            include_str!("../tests/fixtures/en.dic"),
        )
        .unwrap()
    }

    #[test]
    fn checks_words_with_byte_offsets_and_ignores_addresses_and_code() {
        let text = "hello café helo `typpo` https://typpo.test a@typpo.test";
        assert_eq!(
            misspellings(text, &dictionary(), &HashSet::new()),
            vec![(12, "helo")]
        );
        assert!(misspellings("helo", &dictionary(), &HashSet::from(["helo".into()])).is_empty());
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
