//! The reader for Python.
//!
//! Python blocks are indentation, not braces. A dialect supplies the comment
//! mark, the identifier characters, and the keywords that open a named block.
//! A leading indent change is a block boundary. Trailing space is formatting.

use crate::classify::NamedBlock;
use crate::dialect::{BlockCategory, BlockEdge, Dialect, IdentifierCharacters, NamedBlockKeyword};

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

/// The named block added by `text`, when this dialect's keywords say so.
///
/// The first line is read for the keyword and the name. Leading spaces and
/// tabs are skipped, then any listed qualifiers, then a keyword, then the
/// identifier that names the block. A qualifier is matched as written. The
/// keyword selects the category. The whole text then has to be that one
/// indented block: a later line is blank or indented further than the header,
/// and at least one line is indented further. A comment, a dialect with no
/// such keywords, a header without its body, and a run of two blocks have no
/// block. A decorator above the keyword is not skipped.
pub(super) fn named_block(dialect: &Dialect, text: &str) -> Option<NamedBlock> {
    if dialect.named_block_keywords.is_empty() {
        return None;
    }
    let line = text.split('\n').next().unwrap_or("");
    if line.trim_start().starts_with(dialect.line_comment) {
        return None;
    }
    let opened = block_name(dialect, line)?;
    if !one_block(dialect, text) {
        return None;
    }
    Some(NamedBlock {
        category: opened.category,
        name: opened.name.to_string(),
        enclosing: None,
    })
}

/// Whether `following` is still inside the indented block opened by `addition`.
///
/// A blank line does not answer the question. A line indented further than
/// the header is still in the block. A line at the header's indent or less
/// is the next block, or code after it.
pub(super) fn block_resumes(addition: &str, following: &str) -> bool {
    let Some(header) = addition.split('\n').next() else {
        return false;
    };
    let Some(line) = following.split('\n').next() else {
        return false;
    };
    if only_ascii_blank(line) {
        return false;
    }
    leading_ascii_blank(line).len() > leading_ascii_blank(header).len()
}

/// Whether `text` is one complete indented block.
///
/// The first line is the header. A later line that is not blank has to be
/// indented further, and at least one such line has to be present. A blank
/// line stays inside the block. A line that returns to the header's indent,
/// or less, means the text continues past the block.
fn one_block(dialect: &Dialect, text: &str) -> bool {
    if dialect.block_start != BlockEdge::Indentation || dialect.block_end != BlockEdge::Indentation
    {
        return false;
    }
    let mut lines = text.split('\n');
    let Some(header) = lines.next() else {
        return false;
    };
    let header_indent = leading_ascii_blank(header).len();
    let mut saw_body = false;
    for line in lines {
        if only_ascii_blank(line) {
            continue;
        }
        if leading_ascii_blank(line).len() <= header_indent {
            return false;
        }
        saw_body = true;
    }
    saw_body
}

/// The method scope that `before` leaves open around `addition`.
///
/// Indentation is the block edge. A header that ends with `:` opens a block
/// at its indent. `class` is a method scope and `def` is a callable. The
/// name is the identifier after `class`, so `class Count` and
/// `class Count(Bar)` both name `Count`. An `if` is neither, so it does not
/// hide the class around it. A line that returns to an earlier indent closes
/// the blocks it leaves. The innermost callable or method scope that is still
/// open around `addition` decides.
pub(super) fn enclosing_block(
    dialect: &Dialect,
    before: &str,
    addition: &str,
) -> Option<super::EnclosingBlock> {
    if dialect.block_start != BlockEdge::Indentation || dialect.block_end != BlockEdge::Indentation
    {
        return None;
    }
    let mut stack: Vec<ScopeFrame> = Vec::new();
    for line in before.split('\n') {
        if only_ascii_blank(line) || is_line_comment(dialect, line) {
            continue;
        }
        let indent = leading_ascii_blank(line).len();
        while stack.last().is_some_and(|frame| frame.indent >= indent) {
            stack.pop();
        }
        if opens_block(dialect, line) {
            stack.push(ScopeFrame {
                indent,
                role: frame_role(dialect, line),
            });
        }
    }
    let addition_indent = addition
        .split('\n')
        .next()
        .map(|line| leading_ascii_blank(line).len())
        .unwrap_or(0);
    while stack
        .last()
        .is_some_and(|frame| frame.indent >= addition_indent)
    {
        stack.pop();
    }
    for frame in stack.iter().rev() {
        match &frame.role {
            None => continue,
            Some(super::BlockRole::Callable) => return None,
            Some(super::BlockRole::Method(name)) => {
                return Some(super::EnclosingBlock { name: name.clone() });
            }
        }
    }
    None
}

/// One indented block, and whether it makes an inner callable a method.
///
/// `None` is a block that does not decide, such as `if`.
struct ScopeFrame {
    indent: usize,
    role: Option<super::BlockRole>,
}

/// Whether `line` opens a block, ignoring a trailing line comment.
fn opens_block(dialect: &Dialect, line: &str) -> bool {
    code_of(dialect, line).trim_end().ends_with(':')
}

/// The header on `line`: a method scope, a callable, or neither.
fn frame_role(dialect: &Dialect, line: &str) -> Option<super::BlockRole> {
    let Some((word, after)) = opening_word(dialect, code_of(dialect, line)) else {
        return None;
    };
    if super::keyword_is_method_scope(dialect, word) {
        let name = super::identifier_after(&dialect.identifier_characters, after);
        Some(super::BlockRole::Method(name.map(str::to_string)))
    } else if super::keyword_is_callable(dialect, word) {
        Some(super::BlockRole::Callable)
    } else {
        None
    }
}

/// `line` with its line comment removed.
fn code_of<'a>(dialect: &Dialect, line: &'a str) -> &'a str {
    if dialect.line_comment.is_empty() {
        return line;
    }
    match line.find(dialect.line_comment) {
        Some(index) => &line[..index],
        None => line,
    }
}

/// The first word of `line` after any listed qualifiers, and the text after it.
fn opening_word<'a>(dialect: &Dialect, line: &'a str) -> Option<(&'a str, &'a str)> {
    let mut rest = trim_ascii_blank_start(line);
    loop {
        if let Some(qualifier) =
            super::matching_qualifier(&dialect.identifier_characters, dialect.block_prefixes, rest)
        {
            let after = &rest[qualifier.len()..];
            let next = trim_ascii_blank_start(after);
            if next.len() == after.len() {
                return None;
            }
            rest = next;
            continue;
        }
        return next_word(&dialect.identifier_characters, rest);
    }
}

/// The category and name of the block opened by `line`.
fn block_name<'a>(dialect: &Dialect, line: &'a str) -> Option<OpenedBlock<'a>> {
    let mut rest = trim_ascii_blank_start(line);
    loop {
        if let Some((word, after)) = next_word(&dialect.identifier_characters, rest) {
            if let Some(category) = keyword_category(dialect.named_block_keywords, word) {
                let name_at = trim_ascii_blank_start(after);
                if name_at.len() == after.len() {
                    return None;
                }
                let (name, _) = next_word(&dialect.identifier_characters, name_at)?;
                return Some(OpenedBlock { category, name });
            }
        }
        let Some(qualifier) =
            super::matching_qualifier(&dialect.identifier_characters, dialect.block_prefixes, rest)
        else {
            return None;
        };
        let after = &rest[qualifier.len()..];
        let next = trim_ascii_blank_start(after);
        if next.len() == after.len() {
            return None;
        }
        rest = next;
    }
}

/// A named block found on one line, before the name is copied.
struct OpenedBlock<'a> {
    category: BlockCategory,
    name: &'a str,
}

fn keyword_category(keywords: &[NamedBlockKeyword], word: &str) -> Option<BlockCategory> {
    keywords
        .iter()
        .find(|item| item.keyword == word)
        .map(|item| item.category)
}

/// The next identifier in `line`, and the text that follows it.
fn next_word<'a>(characters: &IdentifierCharacters, line: &'a str) -> Option<(&'a str, &'a str)> {
    let mut chars = line.char_indices();
    let (_, first) = chars.next()?;
    if !characters.starts(first) {
        return None;
    }
    let mut end = first.len_utf8();
    for (index, character) in chars {
        if !characters.continues(character) {
            end = index;
            break;
        }
        end = index + character.len_utf8();
    }
    Some((&line[..end], &line[end..]))
}

fn trim_ascii_blank_start(line: &str) -> &str {
    line.trim_start_matches(is_ascii_blank)
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
