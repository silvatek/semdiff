//! The reader for brace-delimited languages.
//!
//! C, C++, Rust, Java, and JavaScript share this reader. A dialect supplies
//! the comment mark, the identifier characters, and the keywords that open a
//! named block. Braces mark a block, so a change of indentation is formatting.

use crate::classify::NamedBlock;
use crate::dialect::{BlockCategory, Dialect, IdentifierCharacters, NamedBlockKeyword};

/// Whether the edit is only an insignificant margin.
///
/// Braces mark the block, so indentation is formatting. Trailing spaces and
/// tabs are formatting too. The lines have to match once those are removed.
/// A carriage return stays part of the line.
pub(super) fn margin_whitespace(old_line: &str, new_line: &str) -> bool {
    strip_ascii_blank(old_line) == strip_ascii_blank(new_line)
}

/// The named block added by `text`, when this dialect's keywords say so.
///
/// Only the first line is read. Leading spaces and tabs are skipped, then any
/// prefix words, then a keyword, then the identifier that names the block.
/// The keyword selects the category. A comment, a dialect with no such
/// keywords, and a line that does not open in that shape have no block. The
/// remainder of the line, and any following lines, stay the block's text.
pub(super) fn named_block(dialect: &Dialect, text: &str) -> Option<NamedBlock> {
    if dialect.named_block_keywords.is_empty() {
        return None;
    }
    let line = text.split('\n').next().unwrap_or("");
    if line.trim_start().starts_with(dialect.line_comment) {
        return None;
    }
    let opened = block_name(dialect, line)?;
    Some(NamedBlock {
        category: opened.category,
        name: opened.name.to_string(),
    })
}

/// The category and name of the block opened by `line`.
fn block_name<'a>(dialect: &Dialect, line: &'a str) -> Option<OpenedBlock<'a>> {
    let mut rest = trim_ascii_blank_start(line);
    loop {
        let (word, after) = next_word(&dialect.identifier_characters, rest)?;
        if let Some(category) = keyword_category(dialect.named_block_keywords, word) {
            let name_at = trim_ascii_blank_start(after);
            if name_at.len() == after.len() {
                return None;
            }
            let (name, _) = next_word(&dialect.identifier_characters, name_at)?;
            return Some(OpenedBlock { category, name });
        }
        if !is_listed(dialect.block_prefixes, word) {
            return None;
        }
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

fn is_listed(words: &[&str], word: &str) -> bool {
    words.iter().any(|candidate| *candidate == word)
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

fn is_ascii_blank(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// `line` without ASCII spaces and tabs at either end.
fn strip_ascii_blank(line: &str) -> &str {
    line.trim_matches(is_ascii_blank)
}
