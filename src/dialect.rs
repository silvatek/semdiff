//! Languages semdiff can read.
//!
//! A changed line asks which characters form a name. A line whose first
//! non-whitespace characters are the line comment is a full-line comment.
//! Block comments and block edges are recorded for a later step.

#![cfg_attr(not(test), allow(dead_code))]

/// A language semdiff knows how to read.
///
/// Extensions select its files. The identifier characters say which runs of
/// text are names. The line comment flags a full-line comment. Block comments
/// and block edges are recorded so a later comparison can see the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dialect {
    /// Extensions that identify this dialect, including the leading dot.
    pub extensions: &'static [&'static str],
    /// Characters that open a comment running to the end of the line.
    pub line_comment: &'static str,
    /// A comment that can span lines, when this language denotes one.
    pub block_comment: Option<BlockComment>,
    /// How a block of code begins.
    pub block_start: BlockEdge,
    /// How a block of code ends.
    pub block_end: BlockEdge,
    /// Characters that can appear in an identifier.
    pub identifier_characters: IdentifierCharacters,
}

/// ASCII characters that can appear in an identifier.
///
/// `start` can open a name. `continue_with` is every valid character, including
/// `start` and the digits, which cannot open a name. A character outside this
/// list can still belong to a name when Unicode treats it as an identifier
/// character: `XID_Start` to open, `XID_Continue` to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IdentifierCharacters {
    /// ASCII characters that can begin an identifier.
    pub start: &'static str,
    /// ASCII characters that can continue an identifier, including `start`.
    pub continue_with: &'static str,
}

const IDENTIFIER_START: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_";
const IDENTIFIER_CONTINUE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_0123456789";

/// The opening and closing text of a comment that can span lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockComment {
    /// Characters that open the comment.
    pub start: &'static str,
    /// Characters that close the comment.
    pub end: &'static str,
}

/// One edge of a block: where it starts, or where it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockEdge {
    /// A source token, such as `{` or `}`.
    Token(&'static str),
    /// A change of indentation.
    ///
    /// The block starts when a following line is indented further, and ends
    /// when indentation returns to the enclosing level.
    Indentation,
}

/// The Rust dialect.
///
/// Rust files use `.rs`. A line comment starts with `//`, which also covers
/// the doc forms `///` and `//!`. A block comment runs from `/*` to `*/`.
/// A block of code starts with `{` and ends with `}`. An identifier starts
/// with a letter or `_` and continues with letters, digits, or `_`.
pub(crate) static RUST: Dialect = Dialect {
    extensions: &[".rs"],
    line_comment: "//",
    block_comment: Some(BlockComment {
        start: "/*",
        end: "*/",
    }),
    block_start: BlockEdge::Token("{"),
    block_end: BlockEdge::Token("}"),
    identifier_characters: IdentifierCharacters {
        start: IDENTIFIER_START,
        continue_with: IDENTIFIER_CONTINUE,
    },
};

/// The Python dialect.
///
/// Python files use `.py`. A comment starts with `#` and ends at the end of
/// the line. Python does not denote a multi-line comment: a triple-quoted
/// string is a string. A block starts by indenting and ends when the
/// indentation returns. An identifier starts with a letter or `_` and
/// continues with letters, digits, or `_`.
pub(crate) static PYTHON: Dialect = Dialect {
    extensions: &[".py"],
    line_comment: "#",
    block_comment: None,
    block_start: BlockEdge::Indentation,
    block_end: BlockEdge::Indentation,
    identifier_characters: IdentifierCharacters {
        start: IDENTIFIER_START,
        continue_with: IDENTIFIER_CONTINUE,
    },
};

/// Every dialect semdiff knows.
const DIALECTS: &[&Dialect] = &[&RUST, &PYTHON];

/// The dialect for a file path, when semdiff knows that language.
///
/// The path's extension is matched. A file with no known extension has no
/// dialect, so its text is not read as Rust or Python.
pub(crate) fn for_path(path: &str) -> Option<&'static Dialect> {
    DIALECTS.iter().copied().find(|dialect| {
        dialect
            .extensions
            .iter()
            .any(|extension| path.ends_with(extension))
    })
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
    line.bytes().all(|byte| byte == b' ' || byte == b'\t')
}

#[cfg(test)]
mod tests {
    use super::{
        blank_line, for_path, full_line_comment, introduces_line_comment, BlockEdge,
        IdentifierCharacters, PYTHON, RUST,
    };

    fn ascii_identifiers() -> IdentifierCharacters {
        IdentifierCharacters {
            start: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
            continue_with: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_0123456789",
        }
    }

    #[test]
    fn rust_uses_slashes_and_braces() {
        assert_eq!(RUST.extensions, [".rs"]);
        assert_eq!(RUST.line_comment, "//");
        let comment = RUST.block_comment.expect("Rust denotes a block comment");
        assert_eq!(comment.start, "/*");
        assert_eq!(comment.end, "*/");
        assert_eq!(RUST.block_start, BlockEdge::Token("{"));
        assert_eq!(RUST.block_end, BlockEdge::Token("}"));
        assert_eq!(RUST.identifier_characters, ascii_identifiers());
    }

    #[test]
    fn python_uses_hash_and_indentation() {
        assert_eq!(PYTHON.extensions, [".py"]);
        assert_eq!(PYTHON.line_comment, "#");
        assert_eq!(PYTHON.block_comment, None);
        assert_eq!(PYTHON.block_start, BlockEdge::Indentation);
        assert_eq!(PYTHON.block_end, BlockEdge::Indentation);
        assert_eq!(PYTHON.identifier_characters, ascii_identifiers());
    }

    #[test]
    fn a_path_selects_its_dialect() {
        assert_eq!(for_path("src/main.rs"), Some(&RUST));
        assert_eq!(for_path("pkg/__init__.py"), Some(&PYTHON));
        assert_eq!(for_path("notes.txt"), None);
        assert_eq!(for_path("main.rs.bak"), None);
    }

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
}
