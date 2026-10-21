use rustc_ast::token::DocFragmentKind;
use rustc_hir::HirId;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};
use std::ops::Range;

pub struct Document {
    pub text: String,
    pub id: HirId,
    pub included: Vec<std::path::PathBuf>,
    pub comments: Vec<Span>,
    offsets: Vec<Range<usize>>,
}

impl Document {
    pub fn span(&self) -> Span {
        Span::with_root_ctxt(
            BytePos(
                self.offsets
                    .iter()
                    .map(|range| range.start)
                    .min()
                    .unwrap_or(0)
                    .try_into()
                    .unwrap(),
            ),
            BytePos(
                self.offsets
                    .iter()
                    .map(|range| range.end)
                    .max()
                    .unwrap_or(0)
                    .try_into()
                    .unwrap(),
            ),
        )
    }

    pub fn source_range(&self, range: Range<usize>) -> Range<usize> {
        self.offsets[range.start].start..self.offsets[range.end - 1].end
    }
}

pub fn documents(cx: &LateContext<'_>, id: HirId) -> Result<Vec<Document>, String> {
    if !cx.tcx.hir_attrs(id).iter().any(|attribute| {
        matches!(
            attribute.doc_str_and_fragment_kind(),
            Some((_, DocFragmentKind::Raw(_)))
        )
    }) {
        return Ok(Vec::new());
    }
    let mut document = Document {
        text: String::new(),
        id,
        included: Vec::new(),
        comments: Vec::new(),
        offsets: Vec::new(),
    };
    for attribute in cx.tcx.hir_attrs(id) {
        let Some((text, kind)) = attribute.doc_str_and_fragment_kind() else {
            continue;
        };
        let span = match kind {
            DocFragmentKind::Raw(span) => span,
            DocFragmentKind::Sugared(_) => attribute.is_doc_comment().unwrap(),
        };
        let text = text.as_str();
        let source = cx
            .sess()
            .source_map()
            .span_to_snippet(span)
            .map_err(|error| format!("could not read Rustdoc attribute source: {error:?}"))?;
        let (text, offsets) = if matches!(kind, DocFragmentKind::Sugared(_)) {
            let comment = crate::comments::comments(&source)
                .into_iter()
                .next()
                .ok_or("could not map Rustdoc comment")?;
            let offsets = (0..comment.text.len())
                .map(|offset| {
                    let range = comment.source_range(offset..offset + 1);
                    span.lo().0 as usize + range.start..span.lo().0 as usize + range.end
                })
                .collect();
            document.comments.push(span);
            (comment.text, offsets)
        } else {
            let offsets = if source == text {
                // `include_str!` retains a span into the included file.
                let file = cx.sess().source_map().lookup_source_file(span.lo());
                if let rustc_span::FileName::Real(name) = &file.name
                    && let Some(path) = name.local_path()
                {
                    document.included.push(path.to_owned());
                }
                literal_offsets(&source, span.lo())
            } else if let Some(body) = source.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                let mut decoded = String::new();
                let mut offsets = Vec::new();
                let mut invalid = false;
                rustc_literal_escaper::unescape_str(body, |range, result| match result {
                    Ok(character) => {
                        decoded.push(character);
                        offsets.extend(std::iter::repeat_n(
                            span.lo().0 as usize + 1 + range.start
                                ..span.lo().0 as usize + 1 + range.end,
                            character.len_utf8(),
                        ));
                    }
                    Err(error) => invalid |= error.is_fatal(),
                });
                if invalid || decoded != text {
                    return Err("could not map decoded Rustdoc string to its source".into());
                }
                offsets
            } else if source.starts_with('r')
                && let Some(quote) = source.find('"')
                && source[1..quote].bytes().all(|byte| byte == b'#')
            {
                let end = source
                    .len()
                    .checked_sub(quote)
                    .ok_or("invalid raw Rustdoc string")?;
                if end <= quote || &source[quote + 1..end] != text {
                    return Err("could not map raw Rustdoc string to its source".into());
                }
                literal_offsets(text, span.lo() + BytePos((quote + 1).try_into().unwrap()))
            } else {
                // Other expanded expressions can have a call-site span rather than literal text.
                let callsite = span.source_callsite();
                vec![callsite.lo().0 as usize..callsite.hi().0 as usize; text.len()]
            };
            (text.to_owned(), offsets)
        };
        if !document.text.is_empty() {
            document.text.push('\n');
            let boundary = document.offsets.last().unwrap().end;
            document.offsets.push(boundary..boundary);
        }
        document.text.push_str(&text);
        document.offsets.extend(offsets);
    }
    Ok(vec![document])
}

fn literal_offsets(text: &str, start: BytePos) -> Vec<Range<usize>> {
    (0..text.len())
        .map(|offset| start.0 as usize + offset..start.0 as usize + offset + 1)
        .collect()
}
