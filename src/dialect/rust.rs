//! The Rust dialect.

use super::{BlockCategory, Dialect, NamedBlockKeyword};

/// The Rust dialect.
///
/// Rust files use `.rs`. A line comment starts with `//`, which also covers
/// the doc forms `///` and `//!`. A block comment runs from `/*` to `*/`.
/// A string runs from `"` to `"`, and `\` escapes the next character.
/// Rust is a C-family language. A block of code starts with `{` and ends with
/// `}`. An identifier starts with a letter or `_` and continues with letters,
/// digits, or `_`. A named block is opened by `fn`, `struct`, `enum`, `union`,
/// `trait`, or `impl`, optionally after a listed qualifier: `pub`,
/// `pub(crate)`, `pub(super)`, `pub(self)`, `async`, `unsafe`, or `const`.
/// Those keywords are function, structure, enumeration, union, interface, and
/// implementation. A `fn` inside `impl` or `trait` is a method. A `fn` inside
/// another `fn` is a function.
pub(crate) static RUST: Dialect = Dialect {
    extensions: &[".rs"],
    block_prefixes: &[
        "pub(crate)",
        "pub(super)",
        "pub(self)",
        "pub",
        "async",
        "unsafe",
        "const",
    ],
    named_block_keywords: &[
        NamedBlockKeyword {
            keyword: "fn",
            category: BlockCategory::Function,
        },
        NamedBlockKeyword {
            keyword: "struct",
            category: BlockCategory::Structure,
        },
        NamedBlockKeyword {
            keyword: "enum",
            category: BlockCategory::Enumeration,
        },
        NamedBlockKeyword {
            keyword: "union",
            category: BlockCategory::Union,
        },
        NamedBlockKeyword {
            keyword: "trait",
            category: BlockCategory::Interface,
        },
        NamedBlockKeyword {
            keyword: "impl",
            category: BlockCategory::Implementation,
        },
    ],
    method_scopes: &["impl", "trait"],
    unary_minus_keywords: &["return", "break", "if", "while", "match", "in"],
    ..Dialect::baseline()
};
