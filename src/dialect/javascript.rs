//! The JavaScript dialect.

use super::{BlockCategory, Dialect, IdentifierCharacters, NamedBlockKeyword, StringDelimiter};

const IDENTIFIER_START: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
const IDENTIFIER_CONTINUE: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$0123456789";

/// The JavaScript dialect.
///
/// JavaScript files use `.js`. A line comment starts with `//`. A block
/// comment runs from `/*` to `*/`. A string runs between `"`, `'`, or `` ` ``,
/// and `\` escapes the next character. JavaScript is a C-family language. A block
/// of code starts with `{` and ends with `}`. An identifier starts with a
/// letter, `_`, or `$` and continues with letters, digits, `_`, or `$`. A
/// named block is opened by `function` or `class`, which are a function and a
/// class, optionally after a listed qualifier: `export`, `default`, `async`,
/// or `static`. A `function` inside `class` is a method. A `function` inside
/// another `function` is a function. A method written as `step() {` is a
/// c-style function with no type before the name. Inside `class` it is a
/// method. Outside `class` it is a function. `if`, `while`, `for`, `switch`,
/// and `catch` stay line edits. A generator written `function*`, and text
/// above the keyword, stay line edits. The boolean constants are `true` and
/// `false`.
pub(crate) static JAVASCRIPT: Dialect = Dialect {
    extensions: &[".js"],
    identifier_characters: IdentifierCharacters {
        start: IDENTIFIER_START,
        continue_with: IDENTIFIER_CONTINUE,
    },
    block_prefixes: &["export", "default", "async", "static"],
    named_block_keywords: &[
        NamedBlockKeyword {
            keyword: "function",
            category: BlockCategory::Function,
        },
        NamedBlockKeyword {
            keyword: "class",
            category: BlockCategory::Class,
        },
    ],
    method_scopes: &["class"],
    c_style_functions: true,
    statement_keywords: &["if", "while", "for", "switch", "catch"],
    string_delimiters: &[
        StringDelimiter {
            start: "\"",
            end: "\"",
        },
        StringDelimiter {
            start: "'",
            end: "'",
        },
        StringDelimiter {
            start: "`",
            end: "`",
        },
    ],
    unary_minus_keywords: &[
        "return", "throw", "case", "void", "typeof", "delete", "await", "yield", "if", "while",
        "in",
    ],
    boolean_constants: &["true", "false"],
    ..Dialect::baseline()
};
