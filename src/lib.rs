#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

use dylint_linting::config;
use regex::Regex;
use rustc_errors::DiagDecorator;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, Symbol, SyntaxContext};
use std::{
    collections::{BTreeSet, HashSet},
    path::PathBuf,
    sync::LazyLock,
};

mod comments;
mod documents;
mod files;
mod front_matter;
mod markdown;
mod rustdoc;
mod spelling;
use spelling::{Checker, Config};

dylint_linting::dylint_library!();

dylint_linting::declare_lint! {
    /// Checks Rust comments and configured Markdown documents using a Hunspell dictionary.
    /// Markdown code, link destinations, URLs and email addresses are skipped.
    pub SPELLCHECK,
    Warn,
    "misspelled words in comments and Markdown"
}

struct Spellcheck {
    documents: Vec<PathBuf>,
    scopes: Vec<(Span, rustc_hir::HirId)>,
    doc_attributes: Vec<rustdoc::Document>,
    doc_owners: HashSet<rustc_hir::HirId>,
}

impl Spellcheck {
    fn scope(&mut self, cx: &LateContext<'_>, id: rustc_hir::HirId, mut span: Span) {
        if span.from_expansion() {
            return;
        }
        for attribute in cx.tcx.hir_attrs(id) {
            if let Some(doc_span) = attribute.is_doc_comment() {
                span = span.with_lo(span.lo().min(doc_span.lo()));
            }
        }
        self.scopes.push((span, id));
        self.documentation(cx, id);
    }

    fn documentation(&mut self, cx: &LateContext<'_>, id: rustc_hir::HirId) {
        if self.doc_owners.insert(id) {
            match rustdoc::documents(cx, id) {
                Ok(documents) => self.doc_attributes.extend(documents),
                Err(error) => {
                    cx.sess().dcx().err(error);
                }
            }
        }
    }
}

dylint_linting::impl_lint_pass!(Spellcheck => [SPELLCHECK]);

#[unsafe(no_mangle)]
pub fn register_lints(sess: &rustc_session::Session, store: &mut rustc_lint::LintStore) {
    dylint_linting::init_config(sess);
    store.register_lints(&[SPELLCHECK]);
    if !sess.opts.prints.is_empty() {
        return;
    }
    // Dependency information is written before late lint passes; register external inputs now.
    let inputs = (|| -> Result<Vec<PathBuf>, String> {
        let settings = config::<Config>("dylint-spellcheck")
            .map_err(|error| format!("invalid dylint-spellcheck configuration: {error}"))?
            .unwrap_or_default();
        let metadata = workspace_metadata(sess)?;
        let root = metadata.workspace_root.as_std_path();
        let inputs = settings.inputs(root)?;
        let resources = inputs.resource_files();
        for path in inputs.files() {
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
        documents::files(
            sess,
            &metadata,
            &settings.include,
            &settings.exclude,
            &resources,
        )
    })();
    let documents = match inputs {
        Ok(documents) => documents,
        Err(error) => {
            sess.dcx().err(error);
            return;
        }
    };
    store.register_late_lint_pass(Box::new(move |_| {
        Box::new(Spellcheck {
            documents: documents.clone(),
            scopes: Vec::new(),
            doc_attributes: Vec::new(),
            doc_owners: HashSet::new(),
        })
    }));
}

static SKIP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://[^\s<>]+|www\.[^\s<>]+|[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}").unwrap()
});
static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\p{L}\p{M}\p{N}\p{Pc}]+(?:['’][\p{L}\p{M}\p{N}\p{Pc}]+)*").unwrap()
});
static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\p{L}[\p{L}\p{M}]*(?:['’]\p{L}[\p{L}\p{M}]*)*$").unwrap());

fn misspellings<'a>(text: &'a str, checker: &Checker) -> Vec<(std::ops::Range<usize>, &'a str)> {
    let skipped: Vec<_> = SKIP.find_iter(text).map(|m| m.range()).collect();
    TOKEN
        .find_iter(text)
        .filter(|word| !skipped.iter().any(|range| range.contains(&word.start())))
        // Reject a technical token as a whole, not just its numeric or underscore parts.
        .filter(|word| WORD.is_match(word.as_str()))
        .filter(|word| !checker.correct(word.as_str()))
        .map(|word| (word.range(), word.as_str()))
        .collect()
}

#[derive(Default)]
struct Diagnostics {
    candidates: BTreeSet<String>,
    emitted: BTreeSet<(u32, u32)>,
}

fn check_prose(
    cx: &LateContext<'_>,
    prose: &markdown::Prose,
    start: BytePos,
    source_range: impl Fn(std::ops::Range<usize>) -> std::ops::Range<usize>,
    checker: &Checker,
    diagnostics: &mut Diagnostics,
    scopes: &[(Span, rustc_hir::HirId)],
) {
    for (range, word) in misspellings(&prose.text, checker) {
        let range = source_range(prose.source_range(range));
        let lo = start + BytePos(range.start.try_into().unwrap());
        let hi = start + BytePos(range.end.try_into().unwrap());
        let span = Span::new(lo, hi, SyntaxContext::root(), None);
        if diagnostics.emitted.contains(&(lo.0, hi.0)) {
            continue;
        }
        let id = scopes
            .iter()
            .filter(|(scope, _)| scope.contains(span))
            .min_by_key(|(scope, _)| scope.hi() - scope.lo())
            .map_or(rustc_hir::CRATE_HIR_ID, |(_, id)| *id);
        cx.tcx.emit_node_span_lint(
            SPELLCHECK,
            id,
            span,
            DiagDecorator(|diag| {
                diagnostics.emitted.insert((lo.0, hi.0));
                diag.primary_message(format!("misspelled word: `{word}`"));
                if !checker.missing.is_empty() {
                    diagnostics.candidates.insert(word.to_owned());
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

impl<'tcx> LateLintPass<'tcx> for Spellcheck {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        self.documentation(cx, rustc_hir::CRATE_HIR_ID);
    }
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx rustc_hir::Item<'tcx>) {
        self.scope(cx, item.hir_id(), item.span);
        if let rustc_hir::ItemKind::Mod(_, module) = item.kind
            && !item.span.from_expansion()
        {
            let source_map = cx.sess().source_map();
            let file = source_map.lookup_source_file(module.spans.inner_span.lo());
            if file.start_pos != source_map.lookup_source_file(item.span.lo()).start_pos {
                self.scopes.push((
                    Span::new(
                        file.start_pos,
                        file.end_position(),
                        SyntaxContext::root(),
                        None,
                    ),
                    item.hir_id(),
                ));
            }
        }
    }

    fn check_trait_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx rustc_hir::TraitItem<'tcx>) {
        self.scope(cx, item.hir_id(), item.span);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx rustc_hir::ImplItem<'tcx>) {
        self.scope(cx, item.hir_id(), item.span);
    }

    fn check_foreign_item(
        &mut self,
        cx: &LateContext<'tcx>,
        item: &'tcx rustc_hir::ForeignItem<'tcx>,
    ) {
        self.scope(cx, item.hir_id(), item.span);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx rustc_hir::FieldDef<'tcx>) {
        self.scope(cx, field.hir_id, field.span);
    }

    fn check_variant(&mut self, cx: &LateContext<'tcx>, variant: &'tcx rustc_hir::Variant<'tcx>) {
        self.scope(cx, variant.hir_id, variant.span);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx rustc_hir::Expr<'tcx>) {
        self.scope(cx, expr.hir_id, expr.span);
    }

    fn check_block(&mut self, cx: &LateContext<'tcx>, block: &'tcx rustc_hir::Block<'tcx>) {
        self.scope(cx, block.hir_id, block.span);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
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
        let markdown = settings.markdown.clone();
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
        let mut diagnostics = Diagnostics::default();
        let mut included_files = BTreeSet::new();
        for document in &self.doc_attributes {
            let prose = match markdown::prose_with_config(&document.text, &markdown) {
                Ok(prose) => prose,
                Err(error) => {
                    cx.sess().dcx().span_err(document.span(), error);
                    continue;
                }
            };
            if !document.text.is_empty() {
                let span = document.span();
                for path in &document.included {
                    match path.canonicalize() {
                        Ok(path) => {
                            included_files.insert(path);
                        }
                        Err(error) => {
                            cx.sess().dcx().err(format!(
                                "could not resolve included Rustdoc file {}: {error}",
                                path.display()
                            ));
                            return;
                        }
                    }
                }
                check_prose(
                    cx,
                    &prose,
                    BytePos(0),
                    |range| document.source_range(range),
                    &checker,
                    &mut diagnostics,
                    &[(span, document.id)],
                );
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
            for comment in comments::comments(source) {
                if !comment.text.is_empty() {
                    let range = comment.source_range(0..comment.text.len());
                    let span = Span::with_root_ctxt(
                        file.start_pos + BytePos(range.start.try_into().unwrap()),
                        file.start_pos + BytePos(range.end.try_into().unwrap()),
                    );
                    if self
                        .doc_attributes
                        .iter()
                        .any(|document| document.comments.iter().any(|doc| doc.contains(span)))
                    {
                        continue;
                    }
                }
                let prose = match markdown::prose_with_config(&comment.text, &markdown) {
                    Ok(prose) => prose,
                    Err(error) => {
                        cx.sess().dcx().err(format!("{error} in {:?}", file.name));
                        continue;
                    }
                };
                check_prose(
                    cx,
                    &prose,
                    file.start_pos,
                    |range| comment.source_range(range),
                    &checker,
                    &mut diagnostics,
                    &self.scopes,
                );
            }
        }
        for path in &self.documents {
            let canonical = match path.canonicalize() {
                Ok(path) => path,
                Err(error) => {
                    cx.sess().dcx().err(format!(
                        "could not resolve spelling document {}: {error}",
                        path.display()
                    ));
                    return;
                }
            };
            if included_files.contains(&canonical) {
                continue;
            }
            let file = match cx.sess().source_map().load_file(path) {
                Ok(file) => file,
                Err(error) => {
                    cx.sess().dcx().err(format!(
                        "could not read spelling document {}: {error}",
                        path.display()
                    ));
                    return;
                }
            };
            let Some(source) = &file.src else {
                cx.sess().dcx().err(format!(
                    "spelling document has no source text: {}",
                    path.display()
                ));
                return;
            };
            let prose = match documents::prose_with_config(path, source, &markdown) {
                Ok(prose) => prose,
                Err(error) => {
                    cx.sess().dcx().err(error);
                    return;
                }
            };
            check_prose(
                cx,
                &prose,
                file.start_pos,
                |range| range,
                &checker,
                &mut diagnostics,
                &[],
            );
        }
        for vocabulary in &checker.missing {
            match Checker::collect_candidates(vocabulary, &diagnostics.candidates) {
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
    workspace_metadata(sess).map(|metadata| metadata.workspace_root.into_std_path_buf())
}

fn workspace_metadata(sess: &rustc_session::Session) -> Result<cargo_metadata::Metadata, String> {
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
        .map_err(|error| format!("could not resolve spelling configuration directory: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Checker {
        Checker::load(
            Config {
                dictionaries: spelling::Dictionaries::Paths(vec!["tests/fixtures".into()]),
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
    fn technical_tokens_do_not_hide_neighboring_prose_typos() {
        let text = "wrong_token _wrong_ wrong‿token 1st 22nd 3rd 4th \
                    sha256 0xdeadbeef abc123def v2beta １２typpo typpo² \
                    abcdef123456 can't2stop helo-wrld #helo (wrld).";
        let errors = misspellings(text, &checker());
        assert_eq!(
            errors.iter().map(|(_, word)| *word).collect::<Vec<_>>(),
            ["helo", "wrld", "helo", "wrld"]
        );
        for (range, word) in errors {
            assert_eq!(&text[range], word);
        }
    }

    #[test]
    fn unicode_marks_and_apostrophes_keep_complete_source_ranges() {
        let checker = checker();
        for source in [
            "préface cafe\u{301} 'he\u{301}lo' ‘isn’t’ can't dogs’",
            "préface caf&#101;&#x301; 'he&#x301;lo' ‘isn&rsquo;t’ can't dogs’",
        ] {
            let prose = markdown::prose(source);
            let errors = misspellings(&prose.text, &checker);
            assert_eq!(
                errors.iter().map(|(_, word)| *word).collect::<Vec<_>>(),
                [
                    "préface",
                    "cafe\u{301}",
                    "he\u{301}lo",
                    "isn’t",
                    "can't",
                    "dogs"
                ]
            );
            let expected = if source.contains("&#") {
                [
                    "préface",
                    "caf&#101;&#x301;",
                    "he&#x301;lo",
                    "isn&rsquo;t",
                    "can't",
                    "dogs",
                ]
            } else {
                [
                    "préface",
                    "cafe\u{301}",
                    "he\u{301}lo",
                    "isn’t",
                    "can't",
                    "dogs",
                ]
            };
            for ((range, _), expected) in errors.into_iter().zip(expected) {
                assert_eq!(&source[prose.source_range(range)], expected);
            }
        }
    }

    #[test]
    fn comment_token_ranges_survive_decoration_and_markdown_decoding() {
        let source = "/**\n * café cafe\u{301} sha256 wrong_token\n * ‘he&#x301;lo’ **isn’t**\n */";
        let comment = comments::comments(source).pop().unwrap();
        let prose = markdown::prose(&comment.text);
        let errors = misspellings(&prose.text, &checker());
        assert_eq!(
            errors.iter().map(|(_, word)| *word).collect::<Vec<_>>(),
            ["cafe\u{301}", "he\u{301}lo", "isn’t"]
        );
        for ((range, _), expected) in
            errors
                .into_iter()
                .zip(["cafe\u{301}", "he&#x301;lo", "isn’t"])
        {
            assert_eq!(
                &source[comment.source_range(prose.source_range(range))],
                expected
            );
        }
    }

    #[test]
    fn ui() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let settings = format!(
            "[dylint-spellcheck.dictionaries]\nen = [{:?}, {:?}]\n",
            root.join("tests/fixtures"),
            root.join("tests/fixtures/vocabulary.txt"),
        );
        dylint_testing::ui::Test::src_base("dylint_spellcheck", "ui")
            .dylint_toml(&settings)
            .run();
    }
}
