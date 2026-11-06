//! The ANSI C dialect.

use super::{
    BlockComment, BlockEdge, Dialect, Family, IdentifierCharacters, IDENTIFIER_CONTINUE,
    IDENTIFIER_START,
};

/// The ANSI C dialect.
///
/// ANSI C files use `.c` and `.h`. A line comment starts with `//`. That form
/// is a C99 feature. ANSI C and C99 share these extensions, so this dialect
/// treats `//` as a comment. A block comment runs from `/*` to `*/`. ANSI C
/// is a C-family language. A block of code starts with `{` and ends with
/// `}`. An identifier starts with a letter or `_` and continues with letters,
/// digits, or `_`. This dialect records no named blocks, so a function
/// written `int fib(int n) {` stays a line edit.
pub(crate) static ANSI_C: Dialect = Dialect {
    family: Family::CFamily,
    extensions: &[".c", ".h"],
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
    block_prefixes: &[],
    named_block_keywords: &[],
    method_scopes: &[],
};
