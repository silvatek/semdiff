//! Languages semdiff can read.
//!
//! A dialect is configuration. It names the files it covers, how comments and
//! blocks are written, which characters form a name, and which keywords open
//! a named block. `family` selects the reader: brace-delimited languages share
//! the C-family reader, and Python has its own.

#![cfg_attr(not(test), allow(dead_code))]

/// A language semdiff knows how to read.
///
/// The fields are the whole dialect. Extensions select its files. The
/// identifier characters say which runs of text are names. The line comment
/// marks a full-line comment. Block comments and block edges describe the
/// rest of a block. The named-block keywords say which added runs are a new
/// function, method, or structure, and which shared category each keyword is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Dialect {
    /// Which reader interprets this dialect.
    pub family: Family,
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
    /// Qualifiers that may precede the keyword opening a named block.
    ///
    /// Each entry is matched as written and then ignored. `pub async fn` has
    /// the qualifiers `pub` and `async`. `pub(crate)` is one entry, so the
    /// parentheses are not parsed. Text that matches no entry stops the
    /// search, so `pub(in path)` and an attribute stay unrecognized.
    pub block_prefixes: &'static [&'static str],
    /// Keywords that open a named block, each with its category.
    ///
    /// The identifier after the keyword is the block's name. An empty list
    /// means this dialect does not record named blocks. A category that is
    /// absent here is one this dialect does not support.
    pub named_block_keywords: &'static [NamedBlockKeyword],
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

/// Which reader interprets a dialect.
///
/// Brace-delimited languages share one reader. Python is read on its own,
/// because its blocks are indentation rather than braces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// C, C++, Rust, Java, JavaScript, and the other brace-delimited languages.
    CFamily,
    /// Python.
    Python,
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

/// A kind of named block, shared by every dialect.
///
/// A dialect maps its keywords onto this list and leaves out the categories
/// it does not have. `procedure` is a callable that is not a function.
/// `method` is a function or procedure that belongs to a type. `interface`
/// covers a Rust trait and a language that spells the same idea `interface`.
/// `implementation` is a block that supplies a type's methods, such as Rust
/// `impl`. `enumeration` and `union` are type blocks that are not structures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockCategory {
    /// A callable that does not return a value.
    Procedure,
    /// A callable that returns a value and is not attached to a type.
    Function,
    /// A function or procedure that belongs to a type.
    Method,
    /// A product of named fields, such as a Rust struct.
    Structure,
    /// A type that combines data and the operations on it.
    Class,
    /// A type that is one of a fixed set of variants.
    Enumeration,
    /// A type whose fields share one storage location.
    Union,
    /// A set of operations a type can promise to provide.
    Interface,
    /// A block that supplies the operations of a type.
    Implementation,
}

/// Every category a dialect may assign to a named block.
pub(crate) const BLOCK_CATEGORIES: &[BlockCategory] = &[
    BlockCategory::Procedure,
    BlockCategory::Function,
    BlockCategory::Method,
    BlockCategory::Structure,
    BlockCategory::Class,
    BlockCategory::Enumeration,
    BlockCategory::Union,
    BlockCategory::Interface,
    BlockCategory::Implementation,
];

impl BlockCategory {
    /// The semdif value for this category.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Procedure => "procedure",
            Self::Function => "function",
            Self::Method => "method",
            Self::Structure => "structure",
            Self::Class => "class",
            Self::Enumeration => "enumeration",
            Self::Union => "union",
            Self::Interface => "interface",
            Self::Implementation => "implementation",
        }
    }

    /// The category named by a semdif value.
    ///
    /// An unknown word is rejected so a record cannot invent a category.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        BLOCK_CATEGORIES
            .iter()
            .copied()
            .find(|category| category.as_str() == text)
    }
}

/// A keyword that opens a named block, and the category that keyword means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NamedBlockKeyword {
    /// The word in source, such as `fn` or `struct`.
    pub keyword: &'static str,
    /// The shared category that keyword selects.
    pub category: BlockCategory,
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
/// Rust is a C-family language. A block of code starts with `{` and ends with
/// `}`. An identifier starts with a letter or `_` and continues with letters,
/// digits, or `_`. A named block is opened by `fn`, `struct`, `enum`, `union`,
/// `trait`, or `impl`, optionally after a listed qualifier: `pub`,
/// `pub(crate)`, `pub(super)`, `pub(self)`, `async`, `unsafe`, or `const`.
/// Those keywords are function, structure, enumeration, union, interface, and
/// implementation.
/// A method uses `fn` as well, and needs the enclosing type, so a `fn` read
/// from the added lines alone is a function.
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
};

/// The Python dialect.
///
/// Python files use `.py`. A comment starts with `#` and ends at the end of
/// the line. Python is read by its own reader. It does not denote a multi-line
/// comment: a triple-quoted string is a string. A block starts by indenting
/// and ends when the indentation returns. An identifier starts with a letter
/// or `_` and continues with letters, digits, or `_`. A named block is opened
/// by `def` or `class`, which are a function and a class.
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
    use super::{
        for_path, BlockCategory, BlockEdge, Family, IdentifierCharacters, NamedBlockKeyword,
        BLOCK_CATEGORIES, PYTHON, RUST,
    };

    fn ascii_identifiers() -> IdentifierCharacters {
        IdentifierCharacters {
            start: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
            continue_with: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_0123456789",
        }
    }

    #[test]
    fn rust_uses_slashes_and_braces() {
        assert_eq!(RUST.family, Family::CFamily);
        assert_eq!(RUST.extensions, [".rs"]);
        assert_eq!(RUST.line_comment, "//");
        let comment = RUST.block_comment.expect("Rust denotes a block comment");
        assert_eq!(comment.start, "/*");
        assert_eq!(comment.end, "*/");
        assert_eq!(RUST.block_start, BlockEdge::Token("{"));
        assert_eq!(RUST.block_end, BlockEdge::Token("}"));
        assert_eq!(RUST.identifier_characters, ascii_identifiers());
        assert_eq!(
            RUST.block_prefixes,
            [
                "pub(crate)",
                "pub(super)",
                "pub(self)",
                "pub",
                "async",
                "unsafe",
                "const",
            ]
        );
        assert_eq!(
            RUST.named_block_keywords,
            [
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
            ]
        );
    }

    #[test]
    fn python_uses_hash_and_indentation() {
        assert_eq!(PYTHON.family, Family::Python);
        assert_eq!(PYTHON.extensions, [".py"]);
        assert_eq!(PYTHON.line_comment, "#");
        assert_eq!(PYTHON.block_comment, None);
        assert_eq!(PYTHON.block_start, BlockEdge::Indentation);
        assert_eq!(PYTHON.block_end, BlockEdge::Indentation);
        assert_eq!(PYTHON.identifier_characters, ascii_identifiers());
        assert_eq!(PYTHON.block_prefixes, [] as [&str; 0]);
        assert_eq!(
            PYTHON.named_block_keywords,
            [
                NamedBlockKeyword {
                    keyword: "def",
                    category: BlockCategory::Function,
                },
                NamedBlockKeyword {
                    keyword: "class",
                    category: BlockCategory::Class,
                },
            ]
        );
    }

    #[test]
    fn every_dialect_shares_one_category_list() {
        let names: Vec<_> = BLOCK_CATEGORIES
            .iter()
            .map(|category| category.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "procedure",
                "function",
                "method",
                "structure",
                "class",
                "enumeration",
                "union",
                "interface",
                "implementation",
            ]
        );
        for category in BLOCK_CATEGORIES {
            assert_eq!(BlockCategory::parse(category.as_str()), Some(*category));
        }
        assert_eq!(BlockCategory::parse("widget"), None);
        assert_eq!(BlockCategory::parse("fn"), None);
    }

    #[test]
    fn a_path_selects_its_dialect() {
        assert_eq!(for_path("src/main.rs"), Some(&RUST));
        assert_eq!(for_path("pkg/__init__.py"), Some(&PYTHON));
        assert_eq!(for_path("notes.txt"), None);
        assert_eq!(for_path("main.rs.bak"), None);
    }
}
