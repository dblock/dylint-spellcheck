use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

pub struct Comment {
    pub text: String,
    offsets: Vec<usize>,
    end: usize,
}

impl Comment {
    pub fn source_range(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        self.offsets[range.start]..self.offsets[range.end - 1] + 1
    }

    fn append(&mut self, source: &str, range: std::ops::Range<usize>) {
        self.text.push_str(&source[range.clone()]);
        self.offsets.extend(range);
    }
}

pub fn comments(source: &str) -> Vec<Comment> {
    let mut comments: Vec<Comment> = Vec::new();
    let mut previous_line_kind = None;
    let mut end = 0;
    for token in tokenize(source, FrontmatterAllowed::Yes) {
        let start = end;
        end += token.len as usize;
        let (prefix, suffix, line_kind) = match token.kind {
            TokenKind::LineComment { doc_style } => {
                let prefix = if doc_style.is_some() { 3 } else { 2 };
                (prefix, 0, Some(&source[start..start + prefix]))
            }
            TokenKind::BlockComment {
                terminated: true,
                doc_style,
            } => (if doc_style.is_some() { 3 } else { 2 }, 2, None),
            TokenKind::Whitespace => continue,
            _ => {
                previous_line_kind = None;
                continue;
            }
        };
        let mut comment = Comment {
            text: String::new(),
            offsets: Vec::new(),
            end,
        };
        let body_start = start + prefix;
        let body_end = end - suffix;
        let mut offset = body_start;
        for (index, line) in source[body_start..body_end]
            .split_inclusive('\n')
            .enumerate()
        {
            let mut content_start = offset;
            if suffix != 0 && index != 0 {
                let trimmed = line.trim_start_matches([' ', '\t']);
                if trimmed.starts_with('*') {
                    content_start += line.len() - trimmed.len() + 1;
                }
            }
            if (index == 0 || content_start != offset) && source[content_start..].starts_with(' ') {
                content_start += 1;
            }
            comment.append(source, content_start..offset + line.len());
            offset += line.len();
        }
        if let Some(previous) = comments.last_mut()
            && line_kind.is_some()
            && line_kind == previous_line_kind
        {
            let gap = &source[previous.end..start];
            if gap.chars().all(char::is_whitespace)
                && gap.bytes().filter(|&b| b == b'\n').count() <= 1
            {
                if let Some(newline) = gap.find('\n') {
                    previous.append(source, previous.end + newline..previous.end + newline + 1);
                }
                previous.text.push_str(&comment.text);
                previous.offsets.extend(comment.offsets);
                previous.end = end;
                previous_line_kind = line_kind;
                continue;
            }
        }
        comments.push(comment);
        previous_line_kind = line_kind;
    }
    comments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::prose;

    #[test]
    fn combines_adjacent_lines_without_crossing_code_or_comment_kinds() {
        let source = "/// ```\n/// typpo\n/// ```\nfn main() {}\n// helo\n//! wrld\n";
        let comments = comments(source);
        assert_eq!(comments.len(), 3);
        assert!(!prose(&comments[0].text).text.contains("typpo"));
        assert!(prose(&comments[1].text).text.contains("helo"));
        assert!(prose(&comments[2].text).text.contains("wrld"));
    }

    #[test]
    fn strips_block_decoration_without_shifting_unicode_offsets() {
        let source = "/**\n * café helo\n *\n * ```\n * typpo\n * ```\n */";
        let comment = comments(source).pop().unwrap();
        let prose = prose(&comment.text);
        assert!(!prose.text.contains("typpo"));
        let start = prose.text.find("helo").unwrap();
        assert_eq!(
            &source[comment.source_range(prose.source_range(start..start + 4))],
            "helo"
        );
    }
}
