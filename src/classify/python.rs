//! The reader for Python.
//!
//! Python blocks are indentation, not braces. A dialect supplies the comment
//! mark. A leading indent change is a block boundary. Trailing space is
//! formatting. Named blocks are not read yet: `def` and `class` stay line
//! additions until this reader learns them.

use crate::classify::NamedBlock;
use crate::dialect::Dialect;

/// Whether the edit is only an insignificant margin.
///
/// The lines have to match once ASCII spaces and tabs are removed from both
/// ends. A leading change is a block boundary, unless both lines are blank or
/// both are full-line comments. Trailing space is insignificant. A carriage
/// return stays part of the line.
pub(super) fn margin_whitespace(dialect: &Dialect, old_line: &str, new_line: &str) -> bool {
    if strip_ascii_blank(old_line) != strip_ascii_blank(new_line) {
        return false;
    }
    if leading_ascii_blank(old_line) == leading_ascii_blank(new_line) {
        return true;
    }
    if only_ascii_blank(old_line) && only_ascii_blank(new_line) {
        return true;
    }
    is_line_comment(dialect, old_line) && is_line_comment(dialect, new_line)
}

/// The named block added by `text`.
///
/// Python does not record one yet. The arguments are the dialect and the
/// added text, ready for `def` and `class`.
pub(super) fn named_block(_dialect: &Dialect, _text: &str) -> Option<NamedBlock> {
    None
}

fn is_line_comment(dialect: &Dialect, line: &str) -> bool {
    line.trim_start().starts_with(dialect.line_comment)
}

fn only_ascii_blank(line: &str) -> bool {
    line.chars().all(is_ascii_blank)
}

fn is_ascii_blank(character: char) -> bool {
    character == ' ' || character == '\t'
}

fn strip_ascii_blank(line: &str) -> &str {
    line.trim_matches(is_ascii_blank)
}

fn leading_ascii_blank(line: &str) -> &str {
    let rest = line.trim_start_matches(is_ascii_blank);
    &line[..line.len() - rest.len()]
}
