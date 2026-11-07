//! Identifying and classifying changes at the span level.
//!
//! `line_changes` calls `find_spans` once it has two copies of a line, and
//! keeps the rule that the line has exactly one. A span is text that differs,
//! widened to the token that contains the edit. Characters that are the same
//! on both lines are not a span. String and comment flags are set while the
//! span is found. This module then classifies that span and builds the record:
//! a boolean spelling or an integer outside a string and a comment, then one
//! identifier whose old name occurs once. An integer or a boolean spelling is
//! recognized in `constant`. One renamed name is recognized in `identifier`.

pub(crate) mod constant;
pub(crate) mod identifier;

use crate::change::{ChangeList, ConstantChanged, IdentifierRenamed, SpanScope};
use crate::dialect::{dialect_for_path, Dialect, StringDelimiter};
use constant::IntegerToken;

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
    let dialect = dialect_for_path(path)?;
    let shielded = span.in_string || span.in_comment;
    if !shielded
        && constant::is_boolean(path, span.old_text)
        && constant::is_boolean(path, span.new_text)
    {
        let occurrence = boolean_occurrence(dialect, span.line, span.start)?;
        return Some(Classification::Constant {
            old_value: span.old_text.to_string(),
            new_value: span.new_text.to_string(),
            occurrence,
        });
    }
    if !shielded && constant::is_integer(span.old_text) && constant::is_integer(span.new_text) {
        let digits = first_digit(span)?;
        let occurrence = constant::digit_run_index(span.line, digits)? + 1;
        return Some(Classification::Constant {
            old_value: span.old_text.to_string(),
            new_value: span.new_text.to_string(),
            occurrence,
        });
    }
    if is_one_identifier(dialect, span.old_text)
        && is_one_identifier(dialect, span.new_text)
        && identifier::occurrence_count(path, span.line, span.old_text) == Some(1)
    {
        return Some(Classification::Rename {
            oldname: span.old_text.to_string(),
            newname: span.new_text.to_string(),
        });
    }
    None
}

/// Records `span` on line `old` of `path` when it classifies.
///
/// `false` means the span does not classify, so the caller records a line
/// modification instead. `old` and `new` are the line numbers from the line diff.
pub(crate) fn record(
    changes: &mut ChangeList,
    path: &str,
    old: usize,
    new: usize,
    span: &Span<'_>,
) -> bool {
    match classify(path, span) {
        Some(Classification::Constant {
            old_value,
            new_value,
            occurrence,
        }) => {
            changes.add(ConstantChanged::new(
                SpanScope::new(path, old),
                new,
                old_value,
                new_value,
                occurrence,
            ));
            true
        }
        Some(Classification::Rename { oldname, newname }) => {
            changes.add(IdentifierRenamed::new(
                SpanScope::new(path, old),
                new,
                oldname,
                newname,
            ));
            true
        }
        None => false,
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
    atoms(dialect, line)
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
fn span_in_string(dialect: &Dialect, line: &str, start: usize, end: usize) -> bool {
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
fn span_in_comment(dialect: &Dialect, line: &str, start: usize, end: usize) -> bool {
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

/// The byte index of a line-comment marker outside a string.
///
/// The marker runs to the end of the line. A marker inside a string is not a
/// comment, so `//` inside `"http://..."` stays part of the string. A block
/// comment is not this marker. A line with no line comment returns `None`.
pub(crate) fn line_comment_marker(dialect: &Dialect, line: &str) -> Option<usize> {
    let marker = dialect.line_comment;
    if marker.is_empty() {
        return None;
    }
    line_regions(dialect, line)
        .comments
        .iter()
        .find_map(|&(from, to)| {
            if to != line.len() {
                return None;
            }
            let at = from.checked_sub(marker.len())?;
            if line.get(at..from) == Some(marker) {
                Some(at)
            } else {
                None
            }
        })
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

/// One changed stretch of a line.
///
/// The text is the token that contains the edit. `line` is the old copy of
/// the whole line, so a later step can count names and constants on it
/// without being handed the line again.
pub(crate) struct Span<'a> {
    /// The old copy of the line this span sits in.
    pub(crate) line: &'a str,
    /// Byte offset of this span on `line`.
    pub(crate) start: usize,
    /// The changed text on the old line.
    pub(crate) old_text: &'a str,
    /// The changed text on the new line.
    pub(crate) new_text: &'a str,
    /// Whether either side of the span lies inside a string.
    pub(crate) in_string: bool,
    /// Whether either side of the span lies inside a comment.
    pub(crate) in_comment: bool,
}

/// The changed spans between the two copies of a line, from left to right.
///
/// A file with no dialect has no spans.
pub(crate) fn find_spans<'a>(path: &str, old_line: &'a str, new_line: &'a str) -> Vec<Span<'a>> {
    let Some(dialect) = dialect_for_path(path) else {
        return Vec::new();
    };
    let old_atoms = atoms(dialect, old_line);
    let new_atoms = atoms(dialect, new_line);
    changed_gaps(&old_atoms, &new_atoms)
        .into_iter()
        .map(|(old_range, new_range)| {
            let (start, old_end) = atom_bounds(&old_atoms, old_range);
            let (new_start, new_end) = atom_bounds(&new_atoms, new_range);
            let in_string = span_in_string(dialect, old_line, start, old_end)
                || span_in_string(dialect, new_line, new_start, new_end);
            let in_comment = span_in_comment(dialect, old_line, start, old_end)
                || span_in_comment(dialect, new_line, new_start, new_end);
            Span {
                line: old_line,
                start,
                old_text: &old_line[start..old_end],
                new_text: &new_line[new_start..new_end],
                in_string,
                in_comment,
            }
        })
        .collect()
}

/// A token or a run of other characters on one line.
///
/// These pieces cover the line so the two copies can be aligned. Only a
/// piece whose text differs becomes a span.
struct Atom<'a> {
    kind: AtomKind,
    start: usize,
    end: usize,
    text: &'a str,
}

/// What an atom is for alignment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AtomKind {
    Text,
    Identifier,
    Integer,
}

/// Byte bounds of the atoms in `range`.
///
/// An empty range is an insertion or deletion. Its position is the end of the
/// preceding atom, or the start of the line when nothing precedes it.
fn atom_bounds(atoms: &[Atom<'_>], range: std::ops::Range<usize>) -> (usize, usize) {
    if range.is_empty() {
        let at = if range.start == 0 {
            0
        } else {
            atoms[range.start - 1].end
        };
        return (at, at);
    }
    (atoms[range.start].start, atoms[range.end - 1].end)
}

/// Index ranges of atoms whose text differs, one range per contiguous run.
fn changed_gaps(
    old_atoms: &[Atom<'_>],
    new_atoms: &[Atom<'_>],
) -> Vec<(std::ops::Range<usize>, std::ops::Range<usize>)> {
    let matches = longest_common_subsequence(old_atoms, new_atoms);
    let mut gaps = Vec::new();
    let mut old_from = 0;
    let mut new_from = 0;
    for matched in &matches {
        if old_from < matched.old_index || new_from < matched.new_index {
            gaps.push((old_from..matched.old_index, new_from..matched.new_index));
        }
        old_from = matched.old_index + 1;
        new_from = matched.new_index + 1;
    }
    if old_from < old_atoms.len() || new_from < new_atoms.len() {
        gaps.push((old_from..old_atoms.len(), new_from..new_atoms.len()));
    }
    gaps
}

/// One atom that occurs in both copies, in the same order.
struct CommonAtom {
    old_index: usize,
    new_index: usize,
}

/// The longest list of equal atoms that occurs in both copies, in order.
fn longest_common_subsequence(old_atoms: &[Atom<'_>], new_atoms: &[Atom<'_>]) -> Vec<CommonAtom> {
    let mut lengths = vec![vec![0; new_atoms.len() + 1]; old_atoms.len() + 1];
    for old_index in 0..old_atoms.len() {
        for new_index in 0..new_atoms.len() {
            lengths[old_index + 1][new_index + 1] =
                if old_atoms[old_index].text == new_atoms[new_index].text {
                    lengths[old_index][new_index] + 1
                } else {
                    lengths[old_index][new_index + 1].max(lengths[old_index + 1][new_index])
                };
        }
    }
    let mut matched = Vec::new();
    let mut old_index = old_atoms.len();
    let mut new_index = new_atoms.len();
    while old_index > 0 && new_index > 0 {
        if old_atoms[old_index - 1].text == new_atoms[new_index - 1].text {
            matched.push(CommonAtom {
                old_index: old_index - 1,
                new_index: new_index - 1,
            });
            old_index -= 1;
            new_index -= 1;
        } else if lengths[old_index - 1][new_index] >= lengths[old_index][new_index - 1] {
            old_index -= 1;
        } else {
            new_index -= 1;
        }
    }
    matched.reverse();
    matched
}

/// Identifiers, integers, and the text between them, covering `line`.
///
/// An integer includes a leading `-` when that minus starts the number.
/// Digits that continue a name stay inside that name.
fn atoms<'a>(dialect: &Dialect, line: &'a str) -> Vec<Atom<'a>> {
    let integers = constant::integer_tokens(dialect, line);
    let mut next_integer = 0;
    let mut found = Vec::new();
    let mut index = 0;
    while index < line.len() {
        if let Some(token) = integers.get(next_integer) {
            if token.start == index {
                found.push(integer_atom(line, token));
                index = token.end;
                next_integer += 1;
                continue;
            }
        }
        let Some(character) = line[index..].chars().next() else {
            break;
        };
        if dialect.identifier_characters.starts(character) {
            let end = end_of_name(dialect, line, index + character.len_utf8());
            found.push(Atom {
                kind: AtomKind::Identifier,
                start: index,
                end,
                text: &line[index..end],
            });
            index = end;
            continue;
        }
        let start = index;
        index += character.len_utf8();
        while index < line.len() {
            if integers
                .get(next_integer)
                .is_some_and(|token| token.start == index)
            {
                break;
            }
            let Some(character) = line[index..].chars().next() else {
                break;
            };
            if dialect.identifier_characters.starts(character) {
                break;
            }
            index += character.len_utf8();
        }
        found.push(Atom {
            kind: AtomKind::Text,
            start,
            end: index,
            text: &line[start..index],
        });
    }
    found
}

/// The integer atom for `token`.
fn integer_atom<'a>(line: &'a str, token: &IntegerToken) -> Atom<'a> {
    Atom {
        kind: AtomKind::Integer,
        start: token.start,
        end: token.end,
        text: &line[token.start..token.end],
    }
}

/// The byte index just after the identifier that starts before `index`.
fn end_of_name(dialect: &Dialect, line: &str, mut index: usize) -> usize {
    while index < line.len() {
        let Some(character) = line[index..].chars().next() else {
            break;
        };
        if !dialect.identifier_characters.continues(character) {
            break;
        }
        index += character.len_utf8();
    }
    index
}

#[cfg(test)]
mod tests {
    use super::classify;
    use super::constant::{boolean_constant, integer_constant};
    use super::find_spans;
    use super::identifier::identifier_rename;
    use super::{span_in_comment, span_in_string};
    use crate::dialect::{ANSI_C, JAVASCRIPT, PYTHON, RUST};

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
