//! How a line is read once a dialect has been selected.
//!
//! A line comment and a blank line use the dialect's configuration directly.
//! A margin and a named block go to the dialect's reader. Brace-delimited
//! languages share the C-family reader. Python has its own.

mod c_family;
mod python;

use crate::dialect::{for_path, BlockCategory, Dialect, Family, IdentifierCharacters};

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
    line.chars()
        .all(|character| character == ' ' || character == '\t')
}

/// Whether changing `old_line` into `new_line` touches only an insignificant margin.
///
/// The dialect's reader decides. In the C family, braces mark the block, so
/// indentation is formatting. In Python, a leading indent change is a block
/// boundary. Trailing space is insignificant in either reader. A file with no
/// dialect is not whitespace. A carriage return stays part of the line.
pub(crate) fn margin_whitespace(path: &str, old_line: &str, new_line: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    match dialect.family {
        Family::CFamily => c_family::margin_whitespace(old_line, new_line),
        Family::Python => python::margin_whitespace(dialect, old_line, new_line),
    }
}

/// The longest listed qualifier that `rest` starts with.
///
/// The match ends on a token boundary, so `pub` matches `pub fn` and does not
/// match `public`. A longer entry wins, so `pub(crate)` is taken ahead of
/// `pub`. The qualifier is the text to skip; the caller reads what follows.
pub(super) fn matching_qualifier<'a>(
    characters: &IdentifierCharacters,
    qualifiers: &[&str],
    rest: &'a str,
) -> Option<&'a str> {
    qualifiers
        .iter()
        .filter(|qualifier| {
            let Some(after) = rest.strip_prefix(*qualifier) else {
                return false;
            };
            match after.chars().next() {
                Some(character) => !characters.continues(character),
                None => true,
            }
        })
        .max_by_key(|qualifier| qualifier.len())
        .map(|qualifier| &rest[..qualifier.len()])
}

/// Whether `following` is still inside the block opened by `addition`.
///
/// Blank lines are not an answer: the caller skips them and asks about the
/// next line. The C-family reader has nothing to add here, because an unclosed
/// brace block is already not a named block. Python uses indentation, so a
/// further-indented line is still in the block.
pub(crate) fn block_resumes(path: &str, addition: &str, following: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    match dialect.family {
        Family::CFamily => false,
        Family::Python => python::block_resumes(addition, following),
    }
}

/// The named block added by `text`, when the dialect's reader sees one.
///
/// `text` is one `line-added` record and may hold several lines. Each reader
/// uses its dialect's keywords, and the text has to be that one complete block.
pub(crate) fn named_block(path: &str, text: &str) -> Option<NamedBlock> {
    let dialect = for_path(path)?;
    match dialect.family {
        Family::CFamily => c_family::named_block(dialect, text),
        Family::Python => python::named_block(dialect, text),
    }
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
            opened(
                "src/main.rs",
                "    fn fib() {\n        let value = 1;\n    }"
            ),
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
            opened("src/main.rs", "pub(crate) fn hidden() {}"),
            Some((BlockCategory::Function, "hidden".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub(super) fn hidden() {}"),
            Some((BlockCategory::Function, "hidden".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub(self) async fn hidden() {}"),
            Some((BlockCategory::Function, "hidden".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "struct Point {\n    x: i32,\n}"),
            Some((BlockCategory::Structure, "Point".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "unsafe impl Foo {\n    fn ready() {}\n}"),
            Some((BlockCategory::Implementation, "Foo".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub enum Color {\n    Red,\n}"),
            Some((BlockCategory::Enumeration, "Color".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "union Raw {\n    n: u32,\n}"),
            Some((BlockCategory::Union, "Raw".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "trait Draw {\n    fn draw();\n}"),
            Some((BlockCategory::Interface, "Draw".to_string()))
        );
        assert_eq!(
            opened(
                "src/main.rs",
                "fn nested() {\n    if ready {\n        work();\n    }\n}"
            ),
            Some((BlockCategory::Function, "nested".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "fn noted() { // }\n}"),
            Some((BlockCategory::Function, "noted".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "fn noted() { /* } */ }"),
            Some((BlockCategory::Function, "noted".to_string()))
        );
        assert_eq!(
            opened("calc.py", "def fib():\n    return 1"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened(
                "calc.py",
                "def fib():\n    if n:\n        return 1\n    return 0"
            ),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("calc.py", "def fib():\n    return 1\n\n    return 2"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("calc.py", "class Point:\n    x = 1"),
            Some((BlockCategory::Class, "Point".to_string()))
        );
    }

    #[test]
    fn an_addition_has_to_be_one_complete_block() {
        assert_eq!(named_block("src/main.rs", "fn fib() {"), None);
        assert_eq!(
            named_block("src/main.rs", "fn fib() {\n    let value = 1;"),
            None
        );
        assert_eq!(named_block("src/main.rs", "fn fib() {\n    // }\n"), None);
        assert_eq!(named_block("src/main.rs", "struct Point {"), None);
        assert_eq!(named_block("src/main.rs", "struct Unit;"), None);
        assert_eq!(named_block("src/main.rs", "fn one() {}\nfn two() {}"), None);
        assert_eq!(named_block("calc.py", "def fib():"), None);
        assert_eq!(named_block("calc.py", "class Point:"), None);
        assert_eq!(
            named_block(
                "calc.py",
                "def one():\n    return 1\ndef two():\n    return 2"
            ),
            None
        );
    }

    #[test]
    fn an_addition_that_does_not_open_a_named_block_stays_unnamed() {
        assert_eq!(named_block("src/main.rs", "let value = 1;"), None);
        assert_eq!(named_block("src/main.rs", "const VALUE: i32 = 1;"), None);
        assert_eq!(named_block("src/main.rs", "public fn hidden() {}"), None);
        assert_eq!(
            named_block("src/main.rs", "pub(in crate::util) fn hidden() {}"),
            None
        );
        assert_eq!(named_block("src/main.rs", "pub(crate)fn hidden() {}"), None);
        assert_eq!(
            named_block("src/main.rs", "#[derive(Debug)]\nstruct Foo {}"),
            None
        );
        assert_eq!(named_block("src/main.rs", "impl<T> Foo {"), None);
        assert_eq!(named_block("src/main.rs", "// fn fake() {}"), None);
        assert_eq!(named_block("src/main.rs", "fn"), None);
        assert_eq!(named_block("src/main.rs", ""), None);
        assert_eq!(named_block("src/main.rs", "\nfn fib() {}"), None);
        assert_eq!(named_block("calc.py", "@decorator\ndef fib():"), None);
        assert_eq!(named_block("notes.txt", "fn fib() {}"), None);
    }
}
