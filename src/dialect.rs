//! Languages semdiff can read.
//!
//! The comparison does not consult these descriptions yet. They record how a
//! language writes comments and blocks, so the next semantic step can ask.

#![cfg_attr(not(test), allow(dead_code))]

/// A language semdiff knows how to read.
///
/// Extensions select its files. The comment markers and the block edges say
/// what the text means, so a later comparison can tell a comment from code
/// and can see where a block starts and ends.
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
}

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
/// A block of code starts with `{` and ends with `}`.
pub(crate) static RUST: Dialect = Dialect {
    extensions: &[".rs"],
    line_comment: "//",
    block_comment: Some(BlockComment {
        start: "/*",
        end: "*/",
    }),
    block_start: BlockEdge::Token("{"),
    block_end: BlockEdge::Token("}"),
};

/// The Python dialect.
///
/// Python files use `.py`. A comment starts with `#` and ends at the end of
/// the line. Python does not denote a multi-line comment: a triple-quoted
/// string is a string. A block starts by indenting and ends when the
/// indentation returns.
pub(crate) static PYTHON: Dialect = Dialect {
    extensions: &[".py"],
    line_comment: "#",
    block_comment: None,
    block_start: BlockEdge::Indentation,
    block_end: BlockEdge::Indentation,
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

#[cfg(test)]
mod tests {
    use super::{for_path, BlockEdge, PYTHON, RUST};

    #[test]
    fn rust_uses_slashes_and_braces() {
        assert_eq!(RUST.extensions, [".rs"]);
        assert_eq!(RUST.line_comment, "//");
        let comment = RUST.block_comment.expect("Rust denotes a block comment");
        assert_eq!(comment.start, "/*");
        assert_eq!(comment.end, "*/");
        assert_eq!(RUST.block_start, BlockEdge::Token("{"));
        assert_eq!(RUST.block_end, BlockEdge::Token("}"));
    }

    #[test]
    fn python_uses_hash_and_indentation() {
        assert_eq!(PYTHON.extensions, [".py"]);
        assert_eq!(PYTHON.line_comment, "#");
        assert_eq!(PYTHON.block_comment, None);
        assert_eq!(PYTHON.block_start, BlockEdge::Indentation);
        assert_eq!(PYTHON.block_end, BlockEdge::Indentation);
    }

    #[test]
    fn a_path_selects_its_dialect() {
        assert_eq!(for_path("src/main.rs"), Some(&RUST));
        assert_eq!(for_path("pkg/__init__.py"), Some(&PYTHON));
        assert_eq!(for_path("notes.txt"), None);
        assert_eq!(for_path("main.rs.bak"), None);
    }
}
