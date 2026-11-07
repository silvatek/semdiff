//! The reader for brace-delimited languages.
//!
//! C, C++, Rust, Java, and JavaScript share this reader. A dialect supplies
//! the comment mark, the identifier characters, the keywords that open a
//! named block, and whether a c-style function needs a type before its name.
//! Braces mark a block, so a change of indentation is formatting.

use crate::classify::NamedBlock;
use crate::dialect::{BlockCategory, BlockEdge, Dialect, IdentifierCharacters, NamedBlockKeyword};

/// Whether the edit is only an insignificant margin.
///
/// Braces mark the block, so indentation is formatting. Trailing spaces and
/// tabs are formatting too. The lines have to match once those are removed.
/// A carriage return stays part of the line.
pub(crate) fn margin_whitespace(old_line: &str, new_line: &str) -> bool {
    strip_ascii_blank(old_line) == strip_ascii_blank(new_line)
}

/// The named block in `text`, when this dialect's keywords or c-style form say so.
///
/// The keyword form reads the first line for a listed qualifier, a keyword,
/// and the name. The c-style form reads an identifier immediately before `(`.
/// `enclosing` is present when the block sits in one of the dialect's method
/// scopes. The whole text then has to be that one block: the dialect's start
/// token opens it, the end token closes it, and only whitespace follows the
/// close. Comments are skipped while the tokens are counted. A quote is not
/// special, so a brace written inside a string is counted. A comment, a
/// header without its body, and a run of two blocks have no block.
pub(super) fn named_block(
    dialect: &Dialect,
    text: &str,
    enclosing: Option<&super::EnclosingBlock>,
) -> Option<NamedBlock> {
    if let Some(opened) = keyword_block(dialect, text) {
        return Some(opened);
    }
    c_style_function(dialect, text, enclosing)
}

/// The keyword block in `text`.
///
/// A dialect with no named-block keywords has none. The first line is read
/// for the keyword and the name, and the text has to be that one block.
fn keyword_block(dialect: &Dialect, text: &str) -> Option<NamedBlock> {
    if dialect.named_block_keywords.is_empty() {
        return None;
    }
    let line = text.split('\n').next().unwrap_or("");
    if !dialect.line_comment.is_empty() && line.trim_start().starts_with(dialect.line_comment) {
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

/// A c-style function in `text`.
///
/// The name is the identifier immediately before the first `(`. A statement
/// keyword in that position is not a function. `c_fun_type_prefix` requires a
/// type before the name, as in `int step(int n) {`, or rejects one, as in
/// JavaScript `step() {`. Inside a method scope the function is a method.
/// A dialect that clears `c_style_functions` does not use this form.
fn c_style_function(
    dialect: &Dialect,
    text: &str,
    enclosing: Option<&super::EnclosingBlock>,
) -> Option<NamedBlock> {
    if !dialect.c_style_functions {
        return None;
    }
    let line = text.split('\n').next().unwrap_or("");
    if !dialect.line_comment.is_empty() && line.trim_start().starts_with(dialect.line_comment) {
        return None;
    }
    if !one_block(dialect, text) || !line_contains_block_start(dialect, line) {
        return None;
    }
    let header = skip_prefixes(dialect, line);
    let (name, before) = identifier_before_paren(&dialect.identifier_characters, header)?;
    if dialect
        .statement_keywords
        .iter()
        .any(|keyword| *keyword == name)
    {
        return None;
    }
    let has_type = contains_identifier(&dialect.identifier_characters, before);
    if has_type != dialect.c_fun_type_prefix {
        return None;
    }
    if let Some(enclosing) = enclosing {
        return Some(NamedBlock {
            category: BlockCategory::Method,
            name: name.to_string(),
            enclosing: enclosing.name.clone(),
        });
    }
    Some(NamedBlock {
        category: BlockCategory::Function,
        name: name.to_string(),
        enclosing: None,
    })
}

/// Whether `line` contains the token that opens a block.
fn line_contains_block_start(dialect: &Dialect, line: &str) -> bool {
    match dialect.block_start {
        BlockEdge::Token(start) => !start.is_empty() && line.contains(start),
        BlockEdge::Indentation => false,
    }
}

/// `line` after leading spaces and listed qualifiers have been skipped.
fn skip_prefixes<'a>(dialect: &Dialect, line: &'a str) -> &'a str {
    let mut rest = trim_ascii_blank_start(line);
    loop {
        let Some(qualifier) =
            super::matching_qualifier(&dialect.identifier_characters, dialect.block_prefixes, rest)
        else {
            return rest;
        };
        let after = &rest[qualifier.len()..];
        let next = trim_ascii_blank_start(after);
        if next.len() == after.len() {
            return rest;
        }
        rest = next;
    }
}

/// The identifier that ends immediately before the first `(`, and the text before it.
///
/// Whitespace may sit between the name and the parenthesis. `int *step(void)`
/// yields `step` and the text `int *`.
fn identifier_before_paren<'a>(
    characters: &IdentifierCharacters,
    header: &'a str,
) -> Option<(&'a str, &'a str)> {
    let paren = header.find('(')?;
    let mut end = paren;
    while end > 0 {
        let last = header[..end].chars().next_back()?;
        if !is_ascii_blank(last) {
            break;
        }
        end -= last.len_utf8();
    }
    let mut start = end;
    for (index, character) in header[..end].char_indices().rev() {
        if !characters.continues(character) {
            break;
        }
        start = index;
    }
    if start == end {
        return None;
    }
    let name = &header[start..end];
    let first = name.chars().next()?;
    if !characters.starts(first) {
        return None;
    }
    Some((name, &header[..start]))
}

/// Whether `text` holds an identifier.
fn contains_identifier(characters: &IdentifierCharacters, text: &str) -> bool {
    let mut rest = text;
    while !rest.is_empty() {
        if next_word(characters, rest).is_some() {
            return true;
        }
        let Some(character) = rest.chars().next() else {
            return false;
        };
        rest = &rest[character.len_utf8()..];
    }
    false
}

/// Whether `text` is one complete block bounded by the dialect's tokens.
///
/// The depth starts at zero. The start token opens the block and the end
/// token closes it. The block has to open, return to zero, and leave only
/// whitespace after that close. A second block, or a close that was never
/// opened, is not one block. A line comment runs to the end of its line. A
/// block comment runs from its start to its end, and an unclosed one is not
/// one block.
fn one_block(dialect: &Dialect, text: &str) -> bool {
    let (BlockEdge::Token(start), BlockEdge::Token(end)) = (dialect.block_start, dialect.block_end)
    else {
        return false;
    };
    if start.is_empty() || end.is_empty() {
        return false;
    }
    let mut rest = text;
    let mut depth = 0usize;
    while !rest.is_empty() {
        if !dialect.line_comment.is_empty() && rest.starts_with(dialect.line_comment) {
            rest = match rest.find('\n') {
                Some(index) => &rest[index + 1..],
                None => "",
            };
            continue;
        }
        if let Some(comment) = dialect.block_comment {
            if !comment.start.is_empty()
                && !comment.end.is_empty()
                && rest.starts_with(comment.start)
            {
                let after = &rest[comment.start.len()..];
                let Some(end_at) = after.find(comment.end) else {
                    return false;
                };
                rest = &after[end_at + comment.end.len()..];
                continue;
            }
        }
        if rest.starts_with(start) {
            depth = depth.saturating_add(1);
            rest = &rest[start.len()..];
            continue;
        }
        if rest.starts_with(end) {
            if depth == 0 {
                return false;
            }
            depth -= 1;
            rest = &rest[end.len()..];
            if depth == 0 {
                return rest.chars().all(is_block_whitespace);
            }
            continue;
        }
        let Some(character) = rest.chars().next() else {
            break;
        };
        rest = &rest[character.len_utf8()..];
    }
    false
}

fn is_block_whitespace(character: char) -> bool {
    character == ' ' || character == '\t' || character == '\n' || character == '\r'
}

/// The method scope that `before` leaves open, when one is still open.
///
/// Braces are counted from the top of `before`, skipping the dialect's
/// comments. A `{` belongs to the header at the start of the line that has
/// not yet opened its block: `impl` and `trait` open a method scope, and
/// `fn` opens a callable. The name is the identifier after that keyword, so
/// `impl Count` names `Count` and `impl Foo for Bar` names `Foo`. An `if`,
/// a `struct`, or a brace in `-> impl Bar` does not. The innermost callable
/// or method scope decides. A quote is not special, so a brace written
/// inside a string is counted.
pub(super) fn enclosing_block(dialect: &Dialect, before: &str) -> Option<super::EnclosingBlock> {
    let (BlockEdge::Token(start), BlockEdge::Token(end)) = (dialect.block_start, dialect.block_end)
    else {
        return None;
    };
    if start.is_empty() || end.is_empty() {
        return None;
    }
    let mut rest = before;
    let mut stack: Vec<Option<super::BlockRole>> = Vec::new();
    let mut in_header = true;
    let mut header_seen = false;
    let mut header_role: Option<super::BlockRole> = None;
    while !rest.is_empty() {
        if rest.starts_with('\n') || rest.starts_with('\r') {
            rest = &rest[1..];
            in_header = true;
            header_seen = false;
            continue;
        }
        if rest.starts_with(' ') || rest.starts_with('\t') {
            rest = &rest[1..];
            continue;
        }
        if !dialect.line_comment.is_empty() && rest.starts_with(dialect.line_comment) {
            rest = match rest.find('\n') {
                Some(index) => &rest[index + 1..],
                None => "",
            };
            in_header = true;
            header_seen = false;
            continue;
        }
        if let Some(comment) = dialect.block_comment {
            if !comment.start.is_empty()
                && !comment.end.is_empty()
                && rest.starts_with(comment.start)
            {
                let after = &rest[comment.start.len()..];
                let Some(end_at) = after.find(comment.end) else {
                    break;
                };
                rest = &after[end_at + comment.end.len()..];
                continue;
            }
        }
        if rest.starts_with(start) {
            stack.push(header_role.take());
            header_seen = false;
            in_header = false;
            rest = &rest[start.len()..];
            continue;
        }
        if rest.starts_with(end) {
            stack.pop();
            header_role = None;
            header_seen = false;
            in_header = true;
            rest = &rest[end.len()..];
            continue;
        }
        if in_header && !header_seen {
            if let Some(qualifier) = super::matching_qualifier(
                &dialect.identifier_characters,
                dialect.block_prefixes,
                rest,
            ) {
                rest = &rest[qualifier.len()..];
                continue;
            }
        }
        if let Some((word, after)) = next_word(&dialect.identifier_characters, rest) {
            if in_header && !header_seen {
                header_role = header_role_of(dialect, word, after);
                header_seen = true;
            }
            in_header = false;
            rest = after;
            continue;
        }
        let Some(character) = rest.chars().next() else {
            break;
        };
        if character == ';' {
            header_role = None;
            header_seen = false;
            in_header = true;
        } else {
            in_header = false;
        }
        rest = &rest[character.len_utf8()..];
    }
    enclosing_of(&stack)
}

/// The role of a header word, including the type name when it is a method scope.
fn header_role_of(dialect: &Dialect, word: &str, after: &str) -> Option<super::BlockRole> {
    if super::keyword_is_method_scope(dialect, word) {
        let name = super::identifier_after(&dialect.identifier_characters, after);
        Some(super::BlockRole::Method(name.map(str::to_string)))
    } else if super::keyword_is_callable(dialect, word) {
        Some(super::BlockRole::Callable)
    } else {
        None
    }
}

/// The innermost callable or method scope still open on `stack`.
fn enclosing_of(stack: &[Option<super::BlockRole>]) -> Option<super::EnclosingBlock> {
    for role in stack.iter().rev() {
        match role {
            None => continue,
            Some(super::BlockRole::Callable) => return None,
            Some(super::BlockRole::Method(name)) => {
                return Some(super::EnclosingBlock { name: name.clone() });
            }
        }
    }
    None
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

fn is_ascii_blank(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// `line` without ASCII spaces and tabs at either end.
fn strip_ascii_blank(line: &str) -> &str {
    line.trim_matches(is_ascii_blank)
}
