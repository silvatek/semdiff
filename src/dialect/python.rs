//! Python's extensions, comments, strings, names, keywords, and boolean spellings.
//! Behavior that uses them lives in a reader.

use super::{BlockCategory, BlockEdge, Dialect, Family, NamedBlockKeyword, StringDelimiter};

/// The Python dialect.
///
/// Python files use `.py`. A comment starts with `#` and ends at the end of
/// the line. Python is read by its own reader. It does not denote a multi-line
/// comment. A string runs between `"""`, `'''`, `"`, or `'`, and the longer
/// marker wins. `\` escapes the next character. A block starts by indenting
/// and ends when the indentation returns. An identifier starts with a letter
/// or `_` and continues with letters, digits, or `_`. A named block is opened
/// by `def` or `class`, which are a function and a class. A `def` inside
/// `class` is a method. A `def` inside another `def` is a function. The
/// boolean constants are `True` and `False`.
pub(crate) static PYTHON: Dialect = Dialect {
    family: Family::Python,
    extensions: &[".py"],
    line_comment: "#",
    block_comment: None,
    block_start: BlockEdge::Indentation,
    block_end: BlockEdge::Indentation,
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
    string_delimiters: &[
        StringDelimiter {
            start: "\"\"\"",
            end: "\"\"\"",
        },
        StringDelimiter {
            start: "'''",
            end: "'''",
        },
        StringDelimiter {
            start: "\"",
            end: "\"",
        },
        StringDelimiter {
            start: "'",
            end: "'",
        },
    ],
    unary_minus_keywords: &[
        "return", "yield", "await", "not", "and", "or", "assert", "if", "elif", "while", "in",
    ],
    boolean_constants: &["True", "False"],
    ..Dialect::baseline()
};
