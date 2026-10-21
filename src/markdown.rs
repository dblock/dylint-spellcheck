use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

pub fn supports(path: &std::path::Path) -> Result<(), String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension)
            if extension.eq_ignore_ascii_case("md")
                || extension.eq_ignore_ascii_case("markdown") =>
        {
            Ok(())
        }
        _ => Err(format!(
            "unsupported spelling input format: {} (supported: .md, .markdown)",
            path.display()
        )),
    }
}

#[derive(Default)]
pub struct Prose {
    pub text: String,
    offsets: Vec<Range<usize>>,
}

impl Prose {
    pub fn source_range(&self, range: Range<usize>) -> Range<usize> {
        self.offsets[range.start].start..self.offsets[range.end - 1].end
    }

    fn boundary(&mut self) {
        self.text.push(' ');
        self.offsets.push(0..0);
    }
}

pub fn prose(source: &str) -> Prose {
    let mut prose = Prose::default();
    let mut code_block = false;
    for (event, range) in Parser::new_ext(
        source,
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES,
    )
    .into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block = true,
            Event::End(TagEnd::CodeBlock) => code_block = false,
            Event::Text(text) if !code_block => {
                // Escapes and entities may be decoded into separate text events.
                let original = &source[range.clone()];
                let offsets = if original == text.as_ref() {
                    (range.start..range.end)
                        .map(|offset| offset..offset + 1)
                        .collect::<Vec<_>>()
                } else {
                    vec![range; text.len()]
                };
                prose.text.push_str(&text);
                prose.offsets.extend(offsets);
            }
            Event::Start(
                Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Link { .. }
                | Tag::Image { .. },
            )
            | Event::End(
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link
                | TagEnd::Image,
            ) => {}
            _ => prose.boundary(),
        }
    }
    prose
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_prose_labels_and_entities_but_not_code_or_destinations() {
        let source = "# hello *world*\n\n[hello](typpo) ![world](typpo)\n\n`typpo`\n\n```rust\nwrongword\n```\n\n    wrongword\n\ncaf&eacute; helo &copy; \\*world\\*";
        let prose = prose(source);
        assert!(!prose.text.contains("typpo"));
        assert!(!prose.text.contains("wrongword"));
        assert!(prose.text.contains("café helo"));
        let start = prose.text.find("café").unwrap();
        assert_eq!(
            &source[prose.source_range(start..start + "café".len())],
            "caf&eacute;"
        );
        let start = prose.text.find("helo").unwrap();
        assert_eq!(&source[prose.source_range(start..start + 4)], "helo");
    }

    #[test]
    fn keeps_words_contiguous_across_inline_formatting() {
        assert!(prose("hel**lo** w&#111;rld").text.contains("hello world"));
    }

    #[test]
    fn maps_decoded_entities_to_the_entire_entity_not_a_matching_substring() {
        let source = "&fjlig;";
        let prose = prose(source);
        assert_eq!(prose.text.trim(), "fj");
        let start = prose.text.find("fj").unwrap();
        assert_eq!(&source[prose.source_range(start..start + 2)], source);
    }
}
