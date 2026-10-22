use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::Deserialize;
use std::ops::Range;

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub front_matter: crate::front_matter::Config,
}

#[derive(Default)]
pub struct Prose {
    pub text: String,
    offsets: Vec<Range<usize>>,
}

impl Prose {
    pub(crate) fn push_mapped(&mut self, ch: char, range: Range<usize>) {
        self.text.push(ch);
        self.offsets
            .extend(std::iter::repeat_n(range, ch.len_utf8()));
    }

    pub(crate) fn append_mapped(
        &mut self,
        other: &Self,
        map: impl Fn(Range<usize>) -> Range<usize>,
    ) {
        self.boundary();
        self.text.push_str(&other.text);
        self.offsets.extend(
            other.offsets.iter().cloned().map(
                |range| {
                    if range.is_empty() { 0..0 } else { map(range) }
                },
            ),
        );
    }

    pub fn literal(source: &str) -> Self {
        Self {
            text: source.to_owned(),
            offsets: (0..source.len()).map(|offset| offset..offset + 1).collect(),
        }
    }

    pub fn source_range(&self, range: Range<usize>) -> Range<usize> {
        self.offsets[range.start].start..self.offsets[range.end - 1].end
    }

    fn boundary(&mut self) {
        self.text.push(' ');
        self.offsets.push(0..0);
    }
}

pub fn prose_with_config(source: &str, config: &Config) -> Result<Prose, String> {
    let (mut front, body_start) = crate::front_matter::extract(source, &config.front_matter)?;
    let body = prose(&source[body_start..]);
    if body_start == 0 {
        return Ok(body);
    }
    front.append_mapped(&body, |range| {
        body_start + range.start..body_start + range.end
    });
    Ok(front)
}

pub fn prose(source: &str) -> Prose {
    let source = without_suppressed_lines(source);
    let source = source.as_str();
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

fn without_suppressed_lines(source: &str) -> String {
    let mut directives = Vec::new();
    for (event, range) in Parser::new(source).into_offset_iter() {
        if let Event::Html(html) | Event::InlineHtml(html) = event
            && let Some(body) = html.trim().strip_prefix("<!--")
            && let Some(body) = body.strip_suffix("-->")
        {
            let action = body.split(" -- ").next().unwrap().trim();
            if matches!(
                action,
                "spellcheck:disable" | "spellcheck:enable" | "spellcheck:todo"
            ) {
                directives.push((range.start, action.to_owned()));
            }
        }
    }
    let mut disabled = false;
    let mut offset = 0;
    let mut output = String::with_capacity(source.len());
    for line in source.split_inclusive('\n') {
        let mut suppress = disabled;
        for (position, action) in &directives {
            if *position < offset || *position >= offset + line.len() {
                continue;
            }
            let inline = !source[offset..*position].trim().is_empty();
            match action.as_str() {
                "spellcheck:todo" => suppress = true,
                "spellcheck:disable" if inline => suppress = true,
                "spellcheck:disable" => {
                    disabled = true;
                    suppress = true;
                }
                "spellcheck:enable" if !inline => disabled = false,
                _ => {}
            }
        }
        if suppress {
            output.extend(
                line.bytes()
                    .map(|byte| if byte == b'\n' { '\n' } else { ' ' }),
            );
        } else {
            output.push_str(line);
        }
        offset += line.len();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directives_suppress_only_their_scope_and_ignore_code() {
        let source = "hello\n<!-- spellcheck:disable -->\ncafé hiddenword\n<!-- spellcheck:enable -->\nworld\nhiddenword <!-- spellcheck:todo -->\nhello\n```\n<!-- spellcheck:disable -->\n```\nvisibleword\n";
        let prose = prose(source);
        assert!(!prose.text.contains("hiddenword"));
        assert!(prose.text.contains("world"));
        let start = prose.text.find("visibleword").unwrap();
        assert_eq!(
            &source[prose.source_range(start..start + 11)],
            "visibleword"
        );
    }

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
