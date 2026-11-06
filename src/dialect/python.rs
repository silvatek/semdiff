//! The Python dialect.

use super::{
    BlockCategory, BlockEdge, Dialect, Family, IdentifierCharacters, NamedBlockKeyword,
    IDENTIFIER_CONTINUE, IDENTIFIER_START,
};

/// The Python dialect.
///
/// Python files use `.py`. A comment starts with `#` and ends at the end of
/// the line. Python is read by its own reader. It does not denote a multi-line
/// comment: a triple-quoted string is a string. A block starts by indenting
/// and ends when the indentation returns. An identifier starts with a letter
/// or `_` and continues with letters, digits, or `_`. A named block is opened
/// by `def` or `class`, which are a function and a class. A `def` inside
/// `class` is a method. A `def` inside another `def` is a function.
pub(crate) static PYTHON: Dialect = Dialect {
    family: Family::Python,
    extensions: &[".py"],
    line_comment: "#",
    block_comment: None,
    block_start: BlockEdge::Indentation,
    block_end: BlockEdge::Indentation,
    identifier_characters: IdentifierCharacters {
        start: IDENTIFIER_START,
        continue_with: IDENTIFIER_CONTINUE,
    },
    block_prefixes: &[],
    named_block_keywords: &[
        NamedBlockKeyword {
            keyword: "def",
            category: BlockCategory::Function,
        },
        NamedBlockKeyword {
            keyword: "class",
            category: BlockCategory::Class,
        },
    ],
    method_scopes: &["class"],
    c_style_functions: false,
    c_fun_type_prefix: false,
    statement_keywords: &[],
};
