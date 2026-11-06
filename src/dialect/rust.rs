//! The Rust dialect.

use super::{
    BlockCategory, BlockComment, BlockEdge, Dialect, Family, IdentifierCharacters,
    NamedBlockKeyword, IDENTIFIER_CONTINUE, IDENTIFIER_START,
};

/// The Rust dialect.
///
/// Rust files use `.rs`. A line comment starts with `//`, which also covers
/// the doc forms `///` and `//!`. A block comment runs from `/*` to `*/`.
/// Rust is a C-family language. A block of code starts with `{` and ends with
/// `}`. An identifier starts with a letter or `_` and continues with letters,
/// digits, or `_`. A named block is opened by `fn`, `struct`, `enum`, `union`,
/// `trait`, or `impl`, optionally after a listed qualifier: `pub`,
/// `pub(crate)`, `pub(super)`, `pub(self)`, `async`, `unsafe`, or `const`.
/// Those keywords are function, structure, enumeration, union, interface, and
/// implementation. A `fn` inside `impl` or `trait` is a method. A `fn` inside
/// another `fn` is a function.
pub(crate) static RUST: Dialect = Dialect {
    family: Family::CFamily,
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
    c_fun_type_prefix: false,
    statement_keywords: &[],
};
