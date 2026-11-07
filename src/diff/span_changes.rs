//! The change record for one span.
//!
//! `line_changes` finds the spans and keeps the rule that the line has exactly
//! one. This module classifies that span and builds the record: a boolean
//! spelling or an integer outside a string and a comment, then one identifier
//! whose old name occurs once.

use crate::change::Change;
use crate::dialect::Dialect;

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

#[cfg(test)]
mod tests {
    use super::super::span::find_spans;
    use super::classify;
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
        assert!(classify("src/main.rs", &found[0]).is_none());
    }
}
