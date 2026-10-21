#![feature(rustc_private)]

extern crate rustc_errors;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

use dylint_linting::config;
use regex::Regex;
use rustc_errors::DiagDecorator;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, Symbol, SyntaxContext};
use std::{collections::BTreeSet, path::PathBuf, sync::LazyLock};

mod comments;
mod markdown;
mod spelling;
use spelling::{Checker, Config};

dylint_linting::dylint_library!();

dylint_linting::declare_lint! {
    /// Checks words in Rust line and block comments using a configured Hunspell dictionary.
    /// Markdown code, link destinations, URLs and email addresses are skipped.
    /// This source-scanning lint currently supports crate-level suppression only.
    pub SPELLCHECK,
    Warn,
    "misspelled words in comments"
}

dylint_linting::declare_lint_pass!(Spellcheck => [SPELLCHECK]);

#[unsafe(no_mangle)]
pub fn register_lints(sess: &rustc_session::Session, store: &mut rustc_lint::LintStore) {
    dylint_linting::init_config(sess);
    store.register_lints(&[SPELLCHECK]);
    // Dep-info is written before late lint passes; register external inputs now.
    let inputs = (|| -> Result<(), String> {
        let settings = config::<Config>("dylint-spellcheck")
            .map_err(|error| format!("invalid dylint-spellcheck configuration: {error}"))?
            .unwrap_or_default();
        let root = configuration_root(sess)?;
        for path in settings
            .vocabulary_files
            .iter()
            .chain(settings.aff.iter())
            .chain(settings.dic.iter())
        {
            let path = root.join(path);
            sess.file_depinfo
                .lock()
                .insert(Symbol::intern(&path.to_string_lossy()));
            if !path.exists()
                && let Some(parent) = path.parent()
            {
                sess.file_depinfo
                    .lock()
                    .insert(Symbol::intern(&parent.to_string_lossy()));
            }
        }
        Ok(())
    })();
    if let Err(error) = inputs {
        sess.dcx().err(error);
        return;
    }
    store.register_late_lint_pass(Box::new(|_| Box::new(Spellcheck)));
}

static SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://[^\s<>]+|www\.[^\s<>]+|[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}").unwrap()
});
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}+(?:['’]\p{L}+)*").unwrap());

fn misspellings<'a>(text: &'a str, checker: &Checker) -> Vec<(std::ops::Range<usize>, &'a str)> {
    let skipped: Vec<_> = SKIP.find_iter(text).map(|m| m.range()).collect();
    WORD.find_iter(text)
        .filter(|word| !skipped.iter().any(|range| range.contains(&word.start())))
        .filter(|word| !checker.correct(word.as_str()))
        .map(|word| (word.range(), word.as_str()))
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
        let root = match configuration_root(cx.sess()) {
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
        let mut candidates = BTreeSet::new();
        for file in cx.sess().source_map().files().iter() {
            if !matches!(&file.name, rustc_span::FileName::Real(name) if name.local_path().is_some_and(|path| path.extension().is_some_and(|extension| extension == "rs")))
            {
                continue;
            }
            let Some(source) = file.src.as_ref() else {
                continue;
            };
            for comment in comments::comments(source) {
                let prose = markdown::prose(&comment.text);
                for (range, word) in misspellings(&prose.text, &checker) {
                    let range = comment.source_range(prose.source_range(range));
                    let lo = file.start_pos + BytePos(range.start.try_into().unwrap());
                    let hi = file.start_pos + BytePos(range.end.try_into().unwrap());
                    let span = Span::new(lo, hi, SyntaxContext::root(), None);
                    cx.emit_span_lint(
                        SPELLCHECK,
                        span,
                        DiagDecorator(|diag| {
                            diag.primary_message(format!("misspelled word: `{word}`"));
                            if !checker.missing.is_empty() {
                                candidates.insert(word.to_owned());
                                return;
                            }
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
        for vocabulary in &checker.missing {
            match Checker::collect_candidates(vocabulary, &candidates) {
                    Ok(path) => cx.sess().dcx().note(format!(
                        "Collected spelling candidates in {}. Review for genuine typos before renaming to {}. This list accumulates across runs; delete it before a fresh collection.",
                        path.strip_prefix(&root).unwrap_or(&path).display(),
                        vocabulary.strip_prefix(&root).unwrap_or(vocabulary).display()
                    )),
                    Err(error) => { cx.sess().dcx().err(error); }
            }
        }
    }
}

fn configuration_root(sess: &rustc_session::Session) -> Result<PathBuf, String> {
    let source = sess
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
        let prose = markdown::prose("hello café helo `typpo` https://typpo.test a@typpo.test");
        let errors = misspellings(&prose.text, &checker());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].1, "helo");
        assert_eq!(prose.source_range(errors[0].0.clone()), 12..16);
        let mut checker = checker();
        checker.ignored_words.insert("helo".into());
        assert!(misspellings("helo", &checker).is_empty());
    }

    #[test]
    fn markdown_words_map_back_to_original_comment_bytes() {
        let source = "    /// café [h&#101;lo](wrongword)\n    /// ```rust\n    /// typpo\n    /// ```\n    /// **wrld**\nfn main() {}";
        let errors: Vec<_> = comments::comments(source)
            .into_iter()
            .flat_map(|comment| {
                let prose = markdown::prose(&comment.text);
                misspellings(&prose.text, &checker())
                    .into_iter()
                    .map(|(range, word)| {
                        (
                            comment.source_range(prose.source_range(range)),
                            word.to_owned(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(errors.len(), 2);
        assert_eq!(
            errors[0],
            (
                source.find("h&#101;lo").unwrap()..source.find("h&#101;lo").unwrap() + 9,
                "helo".into()
            )
        );
        assert_eq!(&source[errors[1].0.clone()], "wrld");
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
