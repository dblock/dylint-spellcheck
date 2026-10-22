use crate::markdown::Prose;
use regex::Regex;
use saphyr_parser::{Event, Parser, ScalarStyle, Tag};
use serde::Deserialize;
use std::{ops::Range, sync::LazyLock};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub include: Option<Vec<String>>,
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            include: None,
            exclude: Vec::new(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        for path in self.include.iter().flatten().chain(&self.exclude) {
            if path.split('.').any(str::is_empty) {
                return Err(format!(
                    "markdown.front_matter paths must be nonempty dotted paths: {path:?}"
                ));
            }
        }
        Ok(())
    }

    fn selects(&self, path: &[String]) -> bool {
        let matches = |pattern: &String| {
            let parts: Vec<_> = pattern.split('.').collect();
            parts.len() <= path.len() && parts.iter().zip(path).all(|(part, key)| *part == key)
        };
        self.include
            .as_ref()
            .is_none_or(|paths| paths.iter().any(matches))
            && !self.exclude.iter().any(matches)
    }
}

pub fn extract(source: &str, config: &Config) -> Result<(Prose, usize), String> {
    config.validate()?;
    let source_start = usize::from(source.starts_with('\u{feff}')) * '\u{feff}'.len_utf8();
    let mut lines = source[source_start..].split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Ok((Prose::default(), 0));
    };
    if first.trim_end_matches([' ', '\t', '\r', '\n']) != "---" {
        return Ok((Prose::default(), 0));
    }
    let start = source_start + first.len();
    let mut end = start;
    for line in lines {
        if matches!(
            line.trim_end_matches([' ', '\t', '\r', '\n']),
            "---" | "..."
        ) {
            let body = end + line.len();
            let prose = values(&source[start..end], start, config)
                .map_err(|error| format!("invalid YAML front matter: {error}"))?;
            return Ok((prose, body));
        }
        end += line.len();
    }
    Err("invalid YAML front matter: missing closing --- or ... delimiter".into())
}

struct Frame {
    path: Option<Vec<String>>,
    mapping: bool,
    expecting_key: bool,
    key: Option<String>,
}

impl Frame {
    fn child(&mut self, scalar: Option<&str>) -> Option<Vec<String>> {
        if !self.mapping {
            return self.path.clone();
        }
        self.expecting_key = !self.expecting_key;
        if !self.expecting_key {
            self.key = scalar.map(str::to_owned);
            None
        } else {
            self.path
                .clone()
                .zip(self.key.take())
                .map(|(mut path, key)| {
                    path.push(key);
                    path
                })
        }
    }
}

fn values(yaml: &str, base: usize, config: &Config) -> Result<Prose, String> {
    let mut prose = Prose::default();
    let mut stack: Vec<Frame> = Vec::new();
    // The iterator input consistently counts characters, unlike accelerated string input.
    // Translate its markers explicitly to `UTF-8` byte positions.
    let positions: Vec<_> = yaml
        .char_indices()
        .map(|(offset, _)| offset)
        .chain([yaml.len()])
        .collect();
    for event in Parser::new_from_iter(yaml.chars()) {
        let (event, span) = event.map_err(|error| error.to_string())?;
        match event {
            Event::Scalar(..)
            | Event::SequenceStart(..)
            | Event::MappingStart(..)
            | Event::Alias(_) => {
                let scalar = match &event {
                    Event::Scalar(value, ..) => Some(value.as_ref()),
                    _ => None,
                };
                let path = stack
                    .last_mut()
                    .map_or_else(|| Some(Vec::new()), |parent| parent.child(scalar));
                match event {
                    Event::Scalar(value, style, _, tag) => {
                        if config.enabled
                            && path.as_ref().is_some_and(|path| config.selects(path))
                            && is_string(&value, style, tag.as_deref())
                        {
                            let range = positions[span.start.index()]..positions[span.end.index()];
                            let scalar = scalar_prose(yaml, range, style, &value)?;
                            let rendered = crate::markdown::prose(&scalar.text);
                            prose.append_mapped(&rendered, |range| {
                                let range = scalar.source_range(range);
                                base + range.start..base + range.end
                            });
                        }
                    }
                    Event::SequenceStart(_, ref tag) | Event::MappingStart(_, ref tag) => {
                        let mapping = matches!(event, Event::MappingStart(..));
                        let path = if tag.as_deref().is_none_or(|tag| {
                            tag.is_yaml_core_schema()
                                && tag.suffix == if mapping { "map" } else { "seq" }
                        }) {
                            path
                        } else {
                            None
                        };
                        stack.push(Frame {
                            path,
                            mapping,
                            expecting_key: true,
                            key: None,
                        });
                    }
                    _ => {}
                }
            }
            Event::SequenceEnd | Event::MappingEnd => {
                stack.pop();
            }
            _ => {}
        }
    }
    Ok(prose)
}

// `YAML` 1.2 core schema resolution is lexical, including numbers too large for machine types.
static NON_STRING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:~|null|Null|NULL|true|True|TRUE|false|False|FALSE|",
        r"[-+]?[0-9]+|0o[0-7]+|0x[0-9a-fA-F]+|",
        r"[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)(?:[eE][-+]?[0-9]+)?|",
        r"[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$"
    ))
    .unwrap()
});

fn is_string(value: &str, style: ScalarStyle, tag: Option<&Tag>) -> bool {
    if let Some(tag) = tag {
        return tag.is_yaml_core_schema() && tag.suffix == "str";
    }
    style != ScalarStyle::Plain || (!value.is_empty() && !NON_STRING.is_match(value))
}

fn scalar_prose(
    yaml: &str,
    range: Range<usize>,
    style: ScalarStyle,
    decoded: &str,
) -> Result<Prose, String> {
    let mut start = range.start;
    let end = range.end;
    if decoded.trim().is_empty() {
        return Ok(Prose::default());
    }
    match style {
        ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted => {
            start += 1;
        }
        _ => {}
    }
    let mut prose = Prose::default();
    let mut cursor = start;
    while cursor < end {
        let first = cursor;
        let mut ch = yaml[cursor..end].chars().next().unwrap();
        cursor += ch.len_utf8();
        if style == ScalarStyle::SingleQuoted && ch == '\'' && yaml[cursor..end].starts_with('\'') {
            cursor += 1;
        } else if (style == ScalarStyle::SingleQuoted && ch == '\'')
            || (style == ScalarStyle::DoubleQuoted && ch == '"')
        {
            break;
        } else if style == ScalarStyle::DoubleQuoted && ch == '\\' {
            ch = yaml[cursor..end]
                .chars()
                .next()
                .ok_or("incomplete YAML escape")?;
            cursor += ch.len_utf8();
            if matches!(ch, '\r' | '\n') {
                while cursor < end
                    && matches!(yaml.as_bytes()[cursor], b' ' | b'\t' | b'\r' | b'\n')
                {
                    cursor += 1;
                }
                continue;
            }
            ch = match ch {
                '0' => '\0',
                'a' => '\u{7}',
                'b' => '\u{8}',
                't' | '\t' => '\t',
                'n' => '\n',
                'v' => '\u{b}',
                'f' => '\u{c}',
                'r' => '\r',
                'e' => '\u{1b}',
                ' ' => ' ',
                '"' => '"',
                '/' => '/',
                '\\' => '\\',
                'N' => '\u{85}',
                '_' => '\u{a0}',
                'L' => '\u{2028}',
                'P' => '\u{2029}',
                'x' | 'u' | 'U' => {
                    let count = match ch {
                        'x' => 2,
                        'u' => 4,
                        _ => 8,
                    };
                    let digits = yaml
                        .get(cursor..cursor + count)
                        .ok_or("incomplete YAML escape")?;
                    cursor += count;
                    u32::from_str_radix(digits, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or("invalid YAML Unicode escape")?
                }
                _ => return Err("unsupported YAML escape".into()),
            };
        }
        prose.push_mapped(ch, first..cursor);
    }
    // Space folding/chomping does not affect words. Never guess locations if the parser
    // scalar span or escape interpretation differs from our byte mapping.
    if prose
        .text
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .ne(decoded.chars().filter(|ch| !ch.is_whitespace()))
    {
        return Err("could not map YAML scalar to exact source bytes".into());
    }
    let mut mapped = Prose::default();
    let mut original = prose
        .text
        .char_indices()
        .filter(|(_, ch)| !ch.is_whitespace());
    for ch in decoded.chars() {
        let range = if ch.is_whitespace() {
            0..0
        } else {
            let (offset, raw) = original.next().ok_or("missing YAML scalar source byte")?;
            prose.source_range(offset..offset + raw.len_utf8())
        };
        mapped.push_mapped(ch, range);
    }
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str, config: &Config) -> Prose {
        crate::markdown::prose_with_config(
            source,
            &crate::markdown::Config {
                front_matter: config.clone(),
            },
        )
        .unwrap()
    }

    #[test]
    fn recursively_checks_only_string_values_and_keeps_body() {
        let source = "---\nignoredkey:\n  - title: helo\n    flags: [true, false, null, 42, 0xFF, .inf, 999999999999999999999999999]\n  - [wrld, 'true', \"false\", !!str null]\ntyped: !!bool false\ncustom: !application ignoredword\nliteral: |-\n    café literalword\nfolded: >+\n  foldedword\n  nextword\n---\nbodyword\n";
        let prose = parse(source, &Config::default());
        for word in [
            "helo",
            "wrld",
            "true",
            "false",
            "null",
            "café",
            "literalword",
            "foldedword",
            "nextword",
            "bodyword",
        ] {
            assert!(prose.text.contains(word), "{word}: {}", prose.text);
        }
        for word in ["ignoredkey", "flags", "FF", "inf", "ignoredword"] {
            assert!(!prose.text.contains(word), "{word}: {}", prose.text);
        }
        let offset = prose.text.find("literalword").unwrap();
        assert_eq!(
            &source[prose.source_range(offset..offset + 11)],
            "literalword"
        );
    }

    #[test]
    fn dotted_paths_select_descendants_and_arrays_without_index_segments() {
        let source = "---\ntitle: titleword\nitems:\n  - title: selectedword\n    hidden: excludedword\n  - title: anotherword\n'items.title': literalkeyword\n---\nbodyword\n";
        let config = Config {
            include: Some(vec!["items".into()]),
            exclude: vec!["items.hidden".into()],
            ..Config::default()
        };
        let prose = parse(source, &config);
        assert!(prose.text.contains("selectedword"));
        assert!(prose.text.contains("anotherword"));
        assert!(prose.text.contains("bodyword"));
        for word in ["titleword", "excludedword", "literalkeyword"] {
            assert!(!prose.text.contains(word), "{}", prose.text);
        }
        for config in [
            Config {
                include: Some(Vec::new()),
                ..Config::default()
            },
            Config {
                enabled: false,
                ..Config::default()
            },
            Config {
                include: Some(vec!["items.title".into()]),
                exclude: vec!["items".into()],
                ..Config::default()
            },
        ] {
            assert_eq!(parse(source, &config).text.trim(), "bodyword");
        }
    }

    #[test]
    fn maps_unicode_escapes_quotes_and_continuations_to_exact_bytes() {
        let source = "---\na: \"café h\\x65lo \\u0077rld \\U00000077rong\"\nb: 'isn''t'\nc: \"hel\\\n  oword\"\nd: \"h\\u00e9lo\"\n---\nbodyword\n";
        let prose = parse(source, &Config::default());
        for (word, raw) in [
            ("helo", "h\\x65lo"),
            ("wrld", "\\u0077rld"),
            ("wrong", "\\U00000077rong"),
            ("isn't", "isn''t"),
            ("heloword", "hel\\\n  oword"),
            ("hélo", "h\\u00e9lo"),
            ("bodyword", "bodyword"),
        ] {
            let offset = prose.text.find(word).unwrap();
            assert_eq!(
                &source[prose.source_range(offset..offset + word.len())],
                raw
            );
        }
    }

    #[test]
    fn accepts_bom_crlf_end_delimiters_and_root_sequences() {
        let source = "\u{feff}--- \r\n- \"hélo\"\r\n- >-\r\n  foldedword\r\n... \r\nbodyword";
        let prose = parse(source, &Config::default());
        for word in ["hélo", "foldedword", "bodyword"] {
            let start = prose.text.find(word).unwrap();
            assert_eq!(&source[prose.source_range(start..start + word.len())], word);
        }
        assert_eq!(
            parse("---\nhelo\n---", &Config::default()).text.trim(),
            "helo"
        );
        assert!(
            parse("before\n---\nbodyword", &Config::default())
                .text
                .contains("before")
        );
    }

    #[test]
    fn composes_yaml_escapes_with_markdown_entities_after_unicode_keys() {
        let source = "---\né: \"h\\x26#101;loword\" # ignoredword\nother: \"isn\\x27t\"\nsingle: 'café'\n---\n";
        let prose = parse(source, &Config::default());
        for (word, raw) in [
            ("heloword", "h\\x26#101;loword"),
            ("isn't", "isn\\x27t"),
            ("café", "café"),
        ] {
            let offset = prose.text.find(word).unwrap();
            assert_eq!(
                &source[prose.source_range(offset..offset + word.len())],
                raw
            );
        }

        assert!(!prose.text.contains("ignoredword"));
    }

    #[test]
    fn empty_scalars_and_whitespace_only_blocks_are_not_mapping_errors() {
        for value in ["''", "\"\"", "'  '", "|", "|+", "|-\n", ">+\n\n", "|\n  \n"] {
            let source = format!("---\nempty: {value}\n---\nbodyword");
            assert_eq!(parse(&source, &Config::default()).text.trim(), "bodyword");
        }
    }

    #[test]
    fn rejects_malformed_yaml_and_invalid_paths_without_falling_back_to_markdown() {
        for source in [
            "---\na: [broken\n---\nbody",
            "---\na: \"\\q\"\n---",
            "---\na: value",
        ] {
            let error =
                crate::markdown::prose_with_config(source, &crate::markdown::Config::default())
                    .err()
                    .unwrap();
            assert!(error.contains("invalid YAML front matter"), "{error}");
        }
        for path in ["", ".a", "a.", "a..b"] {
            let config = Config {
                include: Some(vec![path.into()]),
                ..Config::default()
            };
            assert!(config.validate().is_err());
        }
    }

    #[test]
    fn never_expands_aliases_or_constructs_tagged_objects_and_skips_complex_keys() {
        let source = "---\nname: &name anchorword\nalias: *name\ncustom: !include https://example.invalid/resource\nobject: !application {title: hiddenword}\n? [complexkey]\n: hiddenword\nsafe: visibleword\n---\nbodyword\n";
        let prose = parse(source, &Config::default());
        assert_eq!(prose.text.matches("anchorword").count(), 1);
        assert!(!prose.text.contains("hiddenword"));
        assert!(!prose.text.contains("example"));
        assert!(prose.text.contains("visibleword"));
        assert!(prose.text.contains("bodyword"));
    }

    #[test]
    fn honors_markdown_suppression_inside_scalars_and_in_body() {
        let source = "---\ntitle: 'hiddenword <!-- spellcheck:todo -->'\nsummary: |\n  <!-- spellcheck:disable -->\n  hiddenword\n  <!-- spellcheck:enable -->\n  visibleword\n---\nbodyword\nhiddenword <!-- spellcheck:todo -->\n";
        let prose = parse(source, &Config::default());
        assert!(!prose.text.contains("hiddenword"));
        assert!(prose.text.contains("visibleword"));
        assert!(prose.text.contains("bodyword"));
    }
}
