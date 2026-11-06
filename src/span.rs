//! The changed stretches of one line.
//!
//! A span is text that differs between the two copies of a line, widened to
//! the token that contains the edit. Characters that are the same on both
//! lines are not a span. `find_spans` lists those stretches. `classify` reads
//! one span: a boolean spelling outside a string and a comment, then an
//! integer outside a string and a comment, then one identifier whose old name
//! occurs once.

use crate::classify::{span_in_comment, span_in_string};
use crate::constant::IntegerToken;
use crate::dialect::Dialect;

/// One changed stretch of a line.
///
/// The text is the token that contains the edit.
pub(crate) struct Span<'a> {
    /// Byte offset of this span on the old line.
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

/// The changed spans between the two copies of a line, from left to right.
///
/// A file with no dialect has no spans.
pub(crate) fn find_spans<'a>(path: &str, old_line: &'a str, new_line: &'a str) -> Vec<Span<'a>> {
    let Some(dialect) = crate::dialect::for_path(path) else {
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
                start,
                old_text: &old_line[start..old_end],
                new_text: &new_line[new_start..new_end],
                in_string,
                in_comment,
            }
        })
        .collect()
}

/// The change recorded for `span`.
///
/// A boolean spelling outside a string and a comment is a constant. An integer
/// outside a string and a comment is a constant. One identifier whose old name
/// occurs once on `old_line` is a rename. Anything else is `None`.
pub(crate) fn classify(path: &str, old_line: &str, span: &Span<'_>) -> Option<Classification> {
    let dialect = crate::dialect::for_path(path)?;
    let shielded = span.in_string || span.in_comment;
    if !shielded
        && crate::constant::is_boolean(path, span.old_text)
        && crate::constant::is_boolean(path, span.new_text)
    {
        let occurrence = boolean_occurrence(dialect, old_line, span.start)?;
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
        let occurrence = crate::constant::digit_run_index(old_line, digits)? + 1;
        return Some(Classification::Constant {
            old_value: span.old_text.to_string(),
            new_value: span.new_text.to_string(),
            occurrence,
        });
    }
    if is_one_identifier(dialect, span.old_text)
        && is_one_identifier(dialect, span.new_text)
        && crate::identifier::occurrence_count(path, old_line, span.old_text) == Some(1)
    {
        return Some(Classification::Rename {
            oldname: span.old_text.to_string(),
            newname: span.new_text.to_string(),
        });
    }
    None
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
    let integers = crate::constant::integer_tokens(dialect, line);
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
    use super::{classify, find_spans};
    use crate::constant::{boolean_constant, integer_constant};
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
        assert!(classify("src/main.rs", old_line, &found[0]).is_none());
    }
}
