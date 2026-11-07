//! The ANSI C dialect.

use super::Dialect;

/// The ANSI C dialect.
///
/// ANSI C files use `.c` and `.h`. A line comment starts with `//`. That form
/// is a C99 feature. ANSI C and C99 share these extensions, so this dialect
/// treats `//` as a comment. A block comment runs from `/*` to `*/`. ANSI C
/// is a C-family language. A string runs from `"` to `"`, and `\` escapes the
/// next character. A block of code starts with `{` and ends with
/// `}`. An identifier starts with a letter or `_` and continues with letters,
/// digits, or `_`. A c-style function has a type before its name, as in
/// `int fib(int n) {`, optionally after `static` or `extern`. `step() {` stays
/// a line edit. `if`, `while`, `for`, and `switch` stay line edits. ANSI C
/// lists no boolean constants, so `true` and `false` stay names.
pub(crate) static ANSI_C: Dialect = Dialect {
    extensions: &[".c", ".h"],
    block_prefixes: &["static", "extern"],
    c_style_functions: true,
    c_fun_type_prefix: true,
    statement_keywords: &["if", "while", "for", "switch"],
    unary_minus_keywords: &["return", "case", "sizeof", "if", "while"],
    ..Dialect::baseline()
};
