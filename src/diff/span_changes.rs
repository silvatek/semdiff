//! The change record for one span, and whether a span sits in a string or a comment.
//!
//! `line_changes` finds the spans and keeps the rule that the line has exactly
//! one. This module classifies that span and builds the record: a boolean
//! spelling or an integer outside a string and a comment, then one identifier
//! whose old name occurs once. Finding a span asks this module whether the
//! stretch lies inside a string or a comment that opens on the same line.

use crate::change::Change;
use crate::dialect::{Dialect, StringDelimiter};

use super::span::{AtomKind, Span};

/// The record for one classified span.
pub(crate) enum Classification {
    /// A boolean spelling or an integer changed.
    Constant {
        old_value: String,
        new_value: String,
        occurrence: usize,
    },
    /// One identifier changed, and the old name occurs once on the line.
    Rename { oldname: String, newname: String },
}

/// The change recorded for `span`.
///
/// A boolean spelling outside a string and a comment is a constant. An integer
/// outside a string and a comment is a constant. One identifier whose old name
/// occurs once on the span's line is a rename. Anything else is `None`.
pub(crate) fn classify(path: &str, span: &Span<'_>) -> Option<Classification> {
    let dialect = crate::dialect::for_path(path)?;
    let shielded = span.in_string || span.in_comment;
    if !shielded
        && crate::constant::is_boolean(path, span.old_text)
        && crate::constant::is_boolean(path, span.new_text)
    {
        let occurrence = boolean_occurrence(dialect, span.line, span.start)?;
        return Some(Classification::Constant {
            old_value: span.old_text.to_string(),
            new_value: span.new_text.to_string(),
            occurrence,
        });
    }
    if !shielded
        && crate::constant::is_integer(span.old_text)
        && crate::constant::is_integer(span.new_text)
    {
        let digits = first_digit(span)?;
        let occurrence = crate::constant::digit_run_index(span.line, digits)? + 1;
        return Some(Classification::Constant {
            old_value: span.old_text.to_string(),
            new_value: span.new_text.to_string(),
            occurrence,
        });
    }
    if is_one_identifier(dialect, span.old_text)
        && is_one_identifier(dialect, span.new_text)
        && crate::identifier::occurrence_count(path, span.line, span.old_text) == Some(1)
    {
        return Some(Classification::Rename {
            oldname: span.old_text.to_string(),
            newname: span.new_text.to_string(),
        });
    }
    None
}

/// The semdif record for `span` on line `old` of `path`.
///
/// `None` means the span does not classify, so the caller records a line
/// modification instead. `old` and `new` are the line numbers from the line diff.
pub(crate) fn record(
    path: &str,
    old: usize,
    new: usize,
    span: &Span<'_>,
) -> Option<Box<dyn Change>> {
    match classify(path, span)? {
        Classification::Constant {
            old_value,
            new_value,
            occurrence,
        } => Some(Box::new(crate::change::ConstantChanged::new(
            crate::change::SpanScope::new(path, old),
            new,
            old_value,
            new_value,
            occurrence,
        ))),
        Classification::Rename { oldname, newname } => {
            Some(Box::new(crate::change::IdentifierRenamed::new(
                crate::change::SpanScope::new(path, old),
                new,
                oldname,
                newname,
            )))
        }
    }
}

/// The byte offset of the first digit in `span` on the old line.
fn first_digit(span: &Span<'_>) -> Option<usize> {
    let relative = span
        .old_text
        .char_indices()
        .find(|(_, character)| character.is_ascii_digit())?
        .0;
    Some(span.start + relative)
}

/// Whether `text` is one identifier and nothing else.
fn is_one_identifier(dialect: &Dialect, text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    dialect.identifier_characters.starts(first)
        && characters.all(|character| dialect.identifier_characters.continues(character))
}

/// The 1-based index of the boolean spelling that starts at `start`.
///
/// Spellings inside strings and comments count.
fn boolean_occurrence(dialect: &Dialect, line: &str, start: usize) -> Option<usize> {
    super::span::atoms(dialect, line)
        .iter()
        .filter(|atom| {
            atom.kind == AtomKind::Identifier
                && dialect
                    .boolean_constants
                    .iter()
                    .any(|spelling| *spelling == atom.text)
        })
        .position(|atom| atom.start == start)
        .map(|index| index + 1)
}

/// Whether `line[start..end]` lies inside a string.
///
/// `start` and `end` are byte offsets. The span has to be non-empty and lie
/// entirely between one pair of the dialect's string delimiters. Those
/// delimiters are not inside the string. The longest opening delimiter wins,
/// so `"""` is one string. `string_escape`, when set, makes the next character
/// ordinary text, so `\"` does not close a `"` string. A line comment, and a
/// block comment on this line, hide the quotes inside them. A string or a
/// comment that began on an earlier line is outside this function's view.
pub(super) fn span_in_string(dialect: &Dialect, line: &str, start: usize, end: usize) -> bool {
    span_inside(&line_regions(dialect, line).strings, line, start, end)
}

/// Whether `line[start..end]` lies inside a comment.
///
/// `start` and `end` are byte offsets. The span has to be non-empty and lie
/// entirely inside a comment that opens on this line. The comment markers
/// themselves are outside. A `//` or `#` runs to the end of the line, and a
/// `/*` runs to `*/` or to the end of the line. A marker inside a string does
/// not open a comment. A comment that began on an earlier line is outside
/// this function's view.
pub(super) fn span_in_comment(dialect: &Dialect, line: &str, start: usize, end: usize) -> bool {
    span_inside(&line_regions(dialect, line).comments, line, start, end)
}

/// Whether `[start, end)` lies entirely inside one of `ranges`.
fn span_inside(ranges: &[(usize, usize)], line: &str, start: usize, end: usize) -> bool {
    if start >= end
        || end > line.len()
        || !line.is_char_boundary(start)
        || !line.is_char_boundary(end)
    {
        return false;
    }
    ranges.iter().any(|(from, to)| start >= *from && end <= *to)
}

/// String and comment interiors on one line.
///
/// Each range excludes its delimiters. An unclosed string or block comment
/// runs to the end of the line. A line comment runs from after its marker to
/// the end of the line.
struct LineRegions {
    strings: Vec<(usize, usize)>,
    comments: Vec<(usize, usize)>,
}

/// The string and comment interiors of `line`.
fn line_regions(dialect: &Dialect, line: &str) -> LineRegions {
    let mut regions = LineRegions {
        strings: Vec::new(),
        comments: Vec::new(),
    };
    let mut index = 0;
    while index < line.len() {
        let rest = &line[index..];
        if !dialect.line_comment.is_empty() && rest.starts_with(dialect.line_comment) {
            let interior = index + dialect.line_comment.len();
            if interior < line.len() {
                regions.comments.push((interior, line.len()));
            }
            break;
        }
        if let Some(comment) = dialect.block_comment {
            if !comment.start.is_empty()
                && !comment.end.is_empty()
                && rest.starts_with(comment.start)
            {
                let after = index + comment.start.len();
                if let Some(end_at) = line[after..].find(comment.end) {
                    let close = after + end_at;
                    if after < close {
                        regions.comments.push((after, close));
                    }
                    index = close + comment.end.len();
                } else {
                    if after < line.len() {
                        regions.comments.push((after, line.len()));
                    }
                    break;
                }
                continue;
            }
        }
        let Some(delimiter) = opening_string(dialect, rest) else {
            let Some(character) = rest.chars().next() else {
                break;
            };
            index += character.len_utf8();
            continue;
        };
        let content_at = index + delimiter.start.len();
        match closing_string(dialect, line, content_at, delimiter.end) {
            Some(close_at) => {
                regions.strings.push((content_at, close_at));
                index = close_at + delimiter.end.len();
            }
            None => {
                regions.strings.push((content_at, line.len()));
                break;
            }
        }
    }
    regions
}

/// The longest string delimiter that `rest` starts with.
fn opening_string<'a>(dialect: &'a Dialect, rest: &str) -> Option<&'a StringDelimiter> {
    dialect
        .string_delimiters
        .iter()
        .filter(|delimiter| !delimiter.start.is_empty() && rest.starts_with(delimiter.start))
        .max_by_key(|delimiter| delimiter.start.len())
}

/// The byte offset where `end` closes the string that contains `line[from..]`.
///
/// An escape consumes itself and the next character, so that character cannot
/// start the closing delimiter. No closing delimiter leaves the string open.
fn closing_string(dialect: &Dialect, line: &str, from: usize, end: &str) -> Option<usize> {
    if end.is_empty() {
        return None;
    }
    let mut index = from;
    while index < line.len() {
        let rest = &line[index..];
        if let Some(escape) = dialect.string_escape {
            if rest.starts_with(escape) {
                let after_escape = index + escape.len_utf8();
                let Some(character) = line[after_escape..].chars().next() else {
                    return None;
                };
                index = after_escape + character.len_utf8();
                continue;
            }
        }
        if rest.starts_with(end) {
            return Some(index);
        }
        let Some(character) = rest.chars().next() else {
            break;
        };
        index += character.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::span::find_spans;
    use super::classify;
    use super::{span_in_comment, span_in_string};
    use crate::constant::{boolean_constant, integer_constant};
    use crate::dialect::{ANSI_C, JAVASCRIPT, PYTHON, RUST};
    use crate::identifier::identifier_rename;

    #[test]
    fn an_integer_inside_a_comment_stays_a_modification() {
        assert!(integer_constant("src/main.rs", "let n = 1; // 2", "let n = 1; // 3").is_none());
        assert!(integer_constant("src/main.rs", "/* 1 */", "/* 2 */").is_none());
        assert_eq!(
            integer_constant("src/main.rs", "let n = 1; // 9", "let n = 2; // 9")
                .map(|constant| constant.occurrence),
            Some(1)
        );
        assert_eq!(
            integer_constant("src/main.rs", "/* 9 */ let n = 1;", "/* 9 */ let n = 2;")
                .map(|constant| constant.occurrence),
            Some(2)
        );
    }

    #[test]
    fn a_boolean_inside_a_comment_stays_a_rename() {
        assert!(
            boolean_constant("src/main.rs", "let n = 1; // true", "let n = 1; // false").is_none()
        );
        assert_eq!(
            identifier_rename("src/main.rs", "let n = 1; // true", "let n = 1; // false")
                .map(|rename| (rename.oldname, rename.newname)),
            Some(("true".to_string(), "false".to_string()))
        );
        let old_line = "let n = true; // true";
        let new_line = "let n = true; // false";
        let found = find_spans("src/main.rs", old_line, new_line);
        assert_eq!(found.len(), 1);
        assert!(found[0].in_comment);
        assert!(classify("src/main.rs", &found[0]).is_none());
    }

    #[test]
    fn a_span_inside_a_string_follows_the_dialect() {
        let quoted = "let name = \"héllo\";";
        let (start, end) = at(quoted, "héllo");
        assert!(span_in_string(&RUST, quoted, start, end));
        assert!(span_in_string(&ANSI_C, quoted, start, end));
        let (start, end) = at(quoted, "let");
        assert!(!span_in_string(&RUST, quoted, start, end));
        let (start, end) = at(quoted, "\"héllo\"");
        assert!(!span_in_string(&RUST, quoted, start, end));
        let open_quote = quoted.find('"').unwrap();
        assert!(!span_in_string(&RUST, quoted, open_quote, open_quote + 1));
        let close_quote = quoted.rfind('"').unwrap();
        assert!(!span_in_string(&RUST, quoted, close_quote, close_quote + 1));
        let accent = quoted.find('é').unwrap();
        assert!(!span_in_string(&RUST, quoted, accent + 1, accent + 2));
        assert!(!span_in_string(&RUST, quoted, start, start));
        assert!(!span_in_string(&RUST, quoted, end, start));
        assert!(!span_in_string(&RUST, quoted, 0, quoted.len() + 1));

        let escaped = "let name = \"say \\\"hi\\\"\";";
        let (start, end) = at(escaped, "hi");
        assert!(span_in_string(&RUST, escaped, start, end));
        let across = escaped.find("say").unwrap();
        assert!(span_in_string(&RUST, escaped, across, end));

        let unclosed = "let name = \"hello";
        let (start, end) = at(unclosed, "hello");
        assert!(span_in_string(&RUST, unclosed, start, end));

        let commented = "// \"hello\"";
        let (start, end) = at(commented, "hello");
        assert!(!span_in_string(&RUST, commented, start, end));
        let blocked = "/* \"hello\" */";
        let (start, end) = at(blocked, "hello");
        assert!(!span_in_string(&RUST, blocked, start, end));
        let after = "let name = \"yes\"; // \"no\"";
        let (start, end) = at(after, "yes");
        assert!(span_in_string(&RUST, after, start, end));
        let (start, end) = at(after, "no");
        assert!(!span_in_string(&RUST, after, start, end));
        let hidden = "let name = /* \"no\" */ \"yes\";";
        let (start, end) = at(hidden, "no");
        assert!(!span_in_string(&RUST, hidden, start, end));
        let (start, end) = at(hidden, "yes");
        assert!(span_in_string(&RUST, hidden, start, end));
        let marked = "let name = \"hello // there\";";
        let (start, end) = at(marked, "there");
        assert!(span_in_string(&RUST, marked, start, end));

        let pair = "\"ab\" \"cd\"";
        let (start, end) = at(pair, "ab");
        assert!(span_in_string(&RUST, pair, start, end));
        let (start, end) = at(pair, "cd");
        assert!(span_in_string(&RUST, pair, start, end));
        let across = pair.find('b').unwrap();
        let after_c = pair.find('c').unwrap() + 1;
        assert!(!span_in_string(&RUST, pair, across, after_c));

        let singles = "const name = 'hello';";
        let (start, end) = at(singles, "hello");
        assert!(span_in_string(&JAVASCRIPT, singles, start, end));
        assert!(!span_in_string(&RUST, singles, start, end));
        let ticks = "const name = `hello`;";
        let (start, end) = at(ticks, "hello");
        assert!(span_in_string(&JAVASCRIPT, ticks, start, end));
        assert!(!span_in_string(&RUST, ticks, start, end));

        let triple = "text = \"\"\"hello \"there\"\"\"";
        let (start, end) = at(triple, "there");
        assert!(span_in_string(&PYTHON, triple, start, end));
        assert!(!span_in_string(&RUST, triple, start, end));
        let (start, end) = at(triple, "hello \"there");
        assert!(span_in_string(&PYTHON, triple, start, end));
        let hashed = "# \"hello\"";
        let (start, end) = at(hashed, "hello");
        assert!(!span_in_string(&PYTHON, hashed, start, end));
        let inside_hash = "text = \"hello # there\"";
        let (start, end) = at(inside_hash, "there");
        assert!(span_in_string(&PYTHON, inside_hash, start, end));
    }

    #[test]
    fn a_span_inside_a_comment_follows_the_dialect() {
        let line = "let n = 1; // tail";
        let (start, end) = at(line, "tail");
        assert!(span_in_comment(&RUST, line, start, end));
        let (start, end) = at(line, "1");
        assert!(!span_in_comment(&RUST, line, start, end));
        let marker = line.find("//").unwrap();
        assert!(!span_in_comment(&RUST, line, marker, marker + 2));

        let blocked = "/* tail */ let n = 1;";
        let (start, end) = at(blocked, "tail");
        assert!(span_in_comment(&RUST, blocked, start, end));
        assert!(span_in_comment(&ANSI_C, blocked, start, end));
        let (start, end) = at(blocked, "/*");
        assert!(!span_in_comment(&RUST, blocked, start, end));
        let (start, end) = at(blocked, "*/");
        assert!(!span_in_comment(&RUST, blocked, start, end));
        let (start, end) = at(blocked, "1");
        assert!(!span_in_comment(&RUST, blocked, start, end));

        let unclosed = "/* tail";
        let (start, end) = at(unclosed, "tail");
        assert!(span_in_comment(&RUST, unclosed, start, end));

        let quoted = "let s = \"http://tail\";";
        let (start, end) = at(quoted, "tail");
        assert!(!span_in_comment(&RUST, quoted, start, end));
        assert!(span_in_string(&RUST, quoted, start, end));

        let python = "n = 1  # tail";
        let (start, end) = at(python, "tail");
        assert!(span_in_comment(&PYTHON, python, start, end));
        let (start, end) = at(python, "1");
        assert!(!span_in_comment(&PYTHON, python, start, end));
        let hashed = "text = \"hello # there\"";
        let (start, end) = at(hashed, "there");
        assert!(!span_in_comment(&PYTHON, hashed, start, end));
    }

    fn at(line: &str, text: &str) -> (usize, usize) {
        let start = line.find(text).unwrap();
        (start, start + text.len())
    }
}
