//! How a line is read once a dialect has been selected.
//!
//! A dialect is configuration: comment marks, block edges, identifier
//! characters, and the keywords that open a named block. These functions
//! apply that configuration. A rule for one language is another field on the
//! dialect, not a branch here.

use crate::dialect::{
    for_path, BlockCategory, BlockEdge, Dialect, IdentifierCharacters, NamedBlockKeyword,
};

/// The name and category of one named block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedBlock {
    /// The shared category of this block.
    pub category: BlockCategory,
    /// The identifier that names this block.
    pub name: String,
}

/// Whether `line` introduces this dialect's line comment before any other text.
///
/// Leading whitespace is skipped. The line-comment marker then has to be the
/// next characters. A block comment is not recognized here.
pub(crate) fn introduces_line_comment(dialect: &Dialect, line: &str) -> bool {
    line.trim_start().starts_with(dialect.line_comment)
}

/// Whether `text` is a full-line comment for the file at `path`.
///
/// Every line has to introduce the dialect's line comment. A file with no
/// dialect is not a comment. An empty line is not one either.
pub(crate) fn full_line_comment(path: &str, text: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    if text.is_empty() {
        return false;
    }
    text.split('\n')
        .all(|line| introduces_line_comment(dialect, line))
}

/// Whether `text` is a blank line for the file at `path`.
///
/// A blank line is empty or holds only ASCII spaces and tabs. Rust and Python
/// both ignore that line, so editing one has no functional effect. A file with
/// no dialect is not blank for this purpose. A comment, a carriage return, or
/// any other character keeps the line unflagged.
pub(crate) fn blank_line(path: &str, text: &str) -> bool {
    if for_path(path).is_none() {
        return false;
    }
    text.split('\n').all(only_ascii_blank)
}

/// Whether `line` contains nothing but ASCII spaces and tabs.
fn only_ascii_blank(line: &str) -> bool {
    line.chars().all(is_ascii_blank)
}

/// Whether changing `old_line` into `new_line` touches only an insignificant margin.
///
/// The lines have to match once ASCII spaces and tabs are removed from both
/// ends. A brace dialect treats that indentation as formatting. An indentation
/// dialect treats a leading change as a block boundary, unless both lines are
/// blank or both are full-line comments. Trailing space is insignificant in
/// either dialect. A file with no dialect is not whitespace. A carriage return
/// or any other character stays part of the line.
pub(crate) fn margin_whitespace(path: &str, old_line: &str, new_line: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    if strip_ascii_blank(old_line) != strip_ascii_blank(new_line) {
        return false;
    }
    if leading_ascii_blank(old_line) == leading_ascii_blank(new_line) {
        return true;
    }
    if indent_is_formatting(dialect) {
        return true;
    }
    if only_ascii_blank(old_line) && only_ascii_blank(new_line) {
        return true;
    }
    introduces_line_comment(dialect, old_line) && introduces_line_comment(dialect, new_line)
}

/// The named block added by `text`, when this dialect sees one.
///
/// `text` is one `line-added` record and may hold several lines. Only the
/// first line is read. Leading spaces and tabs are skipped, then any prefix
/// words, then a keyword that opens a named block, then the identifier that
/// names it. The keyword selects the category. A comment, a dialect with no
/// such keywords, and a line that does not open in that shape have no block.
/// The remainder of the line, and any following lines, are kept as the
/// block's text and are not inspected.
pub(crate) fn named_block(path: &str, text: &str) -> Option<NamedBlock> {
    let dialect = for_path(path)?;
    if dialect.named_block_keywords.is_empty() {
        return None;
    }
    let line = text.split('\n').next().unwrap_or("");
    if introduces_line_comment(dialect, line) {
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

/// Whether a block is delimited by tokens, so indentation is formatting.
fn indent_is_formatting(dialect: &Dialect) -> bool {
    matches!(
        (dialect.block_start, dialect.block_end),
        (BlockEdge::Token(_), BlockEdge::Token(_))
    )
}

fn is_ascii_blank(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// `line` without ASCII spaces and tabs at either end.
fn strip_ascii_blank(line: &str) -> &str {
    line.trim_matches(is_ascii_blank)
}

/// The ASCII spaces and tabs at the start of `line`.
fn leading_ascii_blank(line: &str) -> &str {
    let rest = line.trim_start_matches(is_ascii_blank);
    &line[..line.len() - rest.len()]
}

#[cfg(test)]
mod tests {
    use super::{
        blank_line, full_line_comment, introduces_line_comment, margin_whitespace, named_block,
    };
    use crate::dialect::{BlockCategory, PYTHON, RUST};

    #[test]
    fn a_line_comment_is_the_first_non_whitespace() {
        assert!(introduces_line_comment(&RUST, "// note"));
        assert!(introduces_line_comment(&RUST, "    /// doc"));
        assert!(introduces_line_comment(&RUST, "//! inner"));
        assert!(!introduces_line_comment(&RUST, "let x = 1; // note"));
        assert!(!introduces_line_comment(&RUST, "/* block */"));
        assert!(!introduces_line_comment(&RUST, "#[allow(dead_code)]"));
        assert!(!introduces_line_comment(&RUST, ""));
        assert!(!introduces_line_comment(&RUST, "    "));
        assert!(introduces_line_comment(&PYTHON, "# note"));
        assert!(introduces_line_comment(&PYTHON, "  # note"));
        assert!(!introduces_line_comment(&PYTHON, "x = 1  # note"));
        assert!(!introduces_line_comment(&PYTHON, "// not python"));
    }

    #[test]
    fn a_full_line_comment_needs_a_known_dialect() {
        assert!(full_line_comment("src/main.rs", "// note"));
        assert!(full_line_comment("src/main.rs", "// one\n// two"));
        assert!(!full_line_comment("src/main.rs", "// one\nlet x = 1;"));
        assert!(!full_line_comment("src/main.rs", "/* note */"));
        assert!(full_line_comment("calc.py", "# note"));
        assert!(!full_line_comment("notes.txt", "// note"));
        assert!(!full_line_comment("main.rs.bak", "// note"));
        assert!(!full_line_comment("src/main.rs", ""));
    }

    #[test]
    fn a_blank_line_needs_a_known_dialect() {
        assert!(blank_line("src/main.rs", ""));
        assert!(blank_line("src/main.rs", " "));
        assert!(blank_line("src/main.rs", "\t"));
        assert!(blank_line("src/main.rs", " \t "));
        assert!(blank_line("calc.py", ""));
        assert!(blank_line("calc.py", "  "));
        assert!(!blank_line("src/main.rs", "\r"));
        assert!(!blank_line("src/main.rs", " \r"));
        assert!(!blank_line("src/main.rs", "\u{00a0}"));
        assert!(!blank_line("src/main.rs", "let x = 1;"));
        assert!(!blank_line("src/main.rs", "    let x = 1;"));
        assert!(!blank_line("calc.py", "    x = 1"));
        assert!(!blank_line("src/main.rs", "// note"));
        assert!(!blank_line("src/main.rs", "  // note"));
        assert!(!blank_line("calc.py", "# note"));
        assert!(!blank_line("notes.txt", ""));
        assert!(!blank_line("notes.txt", " "));
        assert!(!blank_line("main.rs.bak", " "));
    }

    #[test]
    fn margin_whitespace_follows_the_block_edge() {
        assert!(margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "    let value = 1;"
        ));
        assert!(margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "let value = 1; "
        ));
        assert!(margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "\tlet value = 1;\t"
        ));
        assert!(margin_whitespace("src/main.rs", "// note", "    // note"));
        assert!(margin_whitespace("calc.py", "x = 1", "x = 1 "));
        assert!(margin_whitespace("calc.py", "x = 1", "x = 1\t"));
        assert!(margin_whitespace("calc.py", "# note", "  # note"));
        assert!(margin_whitespace("calc.py", "# note", "# note "));
        assert!(margin_whitespace("src/main.rs", "    ", "\t"));
        assert!(margin_whitespace("calc.py", "", "  "));
        assert!(!margin_whitespace("calc.py", "x = 1", "    x = 1"));
        assert!(!margin_whitespace("calc.py", "x = 1", " x = 1 "));
        assert!(!margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "let  value = 1;"
        ));
        assert!(!margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "let value = 1;\r"
        ));
        assert!(!margin_whitespace("src/main.rs", "    ", "let value = 1;"));
        assert!(!margin_whitespace("notes.txt", "hello", "hello "));
        assert!(!margin_whitespace("notes.txt", "hello", "  hello"));
        assert!(!margin_whitespace(
            "main.rs.bak",
            "let value = 1;",
            "  let value = 1;"
        ));
    }

    fn opened(path: &str, text: &str) -> Option<(BlockCategory, String)> {
        named_block(path, text).map(|block| (block.category, block.name))
    }

    #[test]
    fn a_rust_addition_names_the_block_it_opens() {
        assert_eq!(
            opened("src/main.rs", "fn fib() {}"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "    fn fib() {"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub async fn run() {\n    work();\n}"),
            Some((BlockCategory::Function, "run".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "const fn ready() {}"),
            Some((BlockCategory::Function, "ready".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "struct Point {"),
            Some((BlockCategory::Structure, "Point".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "struct Unit;"),
            Some((BlockCategory::Structure, "Unit".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "unsafe impl Foo {"),
            Some((BlockCategory::Implementation, "Foo".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub enum Color {"),
            Some((BlockCategory::Enumeration, "Color".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "union Raw {"),
            Some((BlockCategory::Union, "Raw".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "trait Draw {"),
            Some((BlockCategory::Interface, "Draw".to_string()))
        );
    }

    #[test]
    fn an_addition_that_does_not_open_a_named_block_stays_unnamed() {
        assert_eq!(named_block("src/main.rs", "let value = 1;"), None);
        assert_eq!(named_block("src/main.rs", "const VALUE: i32 = 1;"), None);
        assert_eq!(
            named_block("src/main.rs", "pub(crate) fn hidden() {}"),
            None
        );
        assert_eq!(
            named_block("src/main.rs", "#[derive(Debug)]\nstruct Foo {}"),
            None
        );
        assert_eq!(named_block("src/main.rs", "impl<T> Foo {"), None);
        assert_eq!(named_block("src/main.rs", "// fn fake() {}"), None);
        assert_eq!(named_block("src/main.rs", "fn"), None);
        assert_eq!(named_block("src/main.rs", ""), None);
        assert_eq!(named_block("src/main.rs", "\nfn fib() {}"), None);
        assert_eq!(named_block("calc.py", "def fib():\n    return 1"), None);
        assert_eq!(named_block("calc.py", "class Point:"), None);
        assert_eq!(named_block("notes.txt", "fn fib() {}"), None);
    }
}
