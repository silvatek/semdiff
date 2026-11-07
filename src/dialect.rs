//! What every language supplies, and which dialect a path selects.
//!
//! A dialect is configuration. It names the files it covers, how comments and
//! blocks are written, which characters form a name, and which keywords open
//! a named block. `family` selects the reader: brace-delimited languages share
//! the C-family reader, and Python has its own. Each language lives in its own
//! file under `dialect/`.

#![cfg_attr(not(test), allow(dead_code))]

/// Every dialect semdiff knows.
///
/// Each static is defined in its language file.
const DIALECTS: &[&Dialect] = &[&RUST, &PYTHON, &JAVASCRIPT, &ANSI_C];

mod ansi_c;
mod javascript;
mod python;
mod rust;

pub(crate) use ansi_c::ANSI_C;
pub(crate) use javascript::JAVASCRIPT;
pub(crate) use python::PYTHON;
pub(crate) use rust::RUST;

/// The dialect for a file path, when semdiff knows that language.
///
/// The path's extension is matched. A file with no known extension has no
/// dialect, so its text is not read as Rust, Python, JavaScript, or ANSI C.
pub(crate) fn for_path(path: &str) -> Option<&'static Dialect> {
    DIALECTS.iter().copied().find(|dialect| {
        dialect
            .extensions
            .iter()
            .any(|extension| path.ends_with(extension))
    })
}

/// A language semdiff knows how to read.
///
/// The fields are the whole dialect. `baseline` fills every field a language
/// static leaves out. Extensions select its files. The
/// identifier characters say which runs of text are names. The line comment
/// marks a full-line comment. Block comments and block edges describe the
/// rest of a block. String delimiters say which spans sit inside a string.
/// The named-block keywords say which added runs are a new
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
    /// Delimiters that open and close a string.
    ///
    /// The longest opening delimiter wins, so `"""` is one Python string and
    /// not three `"` strings. `baseline` lists `"`. Python and JavaScript
    /// replace that list. Rust and ANSI C leave it.
    pub string_delimiters: &'static [StringDelimiter],
    /// The character that escapes the next character inside a string.
    ///
    /// `baseline` uses `\`, so `\"` does not close a `"` string. `None` means
    /// a delimiter ends the string even when a backslash precedes it.
    pub string_escape: Option<char>,
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
    /// Keywords whose block makes an inner callable a method.
    ///
    /// Rust lists `impl` and `trait`. Python and JavaScript list `class`. The
    /// innermost such block wins over an `if` or a bare brace, and loses to a
    /// callable wrapped around the new one. A `fn`, `def`, or `function`
    /// inside another callable stays a function.
    pub method_scopes: &'static [&'static str],
    /// Whether this dialect records a c-style function.
    ///
    /// The name is the identifier immediately before `(`. `baseline` clears
    /// this, so Rust and Python leave it out and a block of that shape stays
    /// a line edit. ANSI C and JavaScript set it.
    pub c_style_functions: bool,
    /// Whether a c-style function has a type before its name.
    ///
    /// `baseline` clears this. JavaScript leaves it out. `step() {` is a
    /// function, and a type before the name is not this form. ANSI C sets it.
    /// `int step(int n) {` is a function, and `step() {` is not. A method
    /// scope around either one records a method.
    pub c_fun_type_prefix: bool,
    /// Words that take the shape of a c-style function and are not one.
    ///
    /// `if (n) {` is a statement. ANSI C and JavaScript each list the words
    /// they use. `baseline` leaves the list empty.
    pub statement_keywords: &'static [&'static str],
    /// Words that do not produce a value, so a following `-` can start a negative integer.
    ///
    /// `return -1` is the integer `-1`. `count-1` is subtraction, because
    /// `count` is not listed. `baseline` lists `return`. A language with more
    /// such words replaces the list.
    pub unary_minus_keywords: &'static [&'static str],
    /// Spellings of the boolean constants.
    ///
    /// A change from one spelling to another is a constant change when that
    /// spelling is the whole name. `baseline` lists none. Rust and JavaScript
    /// list `true` and `false`. Python lists `True` and `False`. ANSI C lists
    /// none, so those words stay names.
    pub boolean_constants: &'static [&'static str],
}

impl Dialect {
    /// The values a language leaves unwritten.
    ///
    /// The shape is the C family: `//` and `/* */` comments, `{` `}` blocks,
    /// a `"` string escaped by `\`, and the shared identifier characters.
    /// Extensions are empty. Prefixes, named-block keywords, method scopes,
    /// and c-style functions are off. `return` can precede a negative integer.
    /// Boolean constants are unlisted. A language static replaces the fields
    /// that differ.
    pub(crate) const fn baseline() -> Self {
        Self {
            family: Family::CFamily,
            extensions: &[],
            line_comment: "//",
            block_comment: Some(BlockComment {
                start: "/*",
                end: "*/",
            }),
            string_delimiters: &[StringDelimiter {
                start: "\"",
                end: "\"",
            }],
            string_escape: Some('\\'),
            block_start: BlockEdge::Token("{"),
            block_end: BlockEdge::Token("}"),
            identifier_characters: IdentifierCharacters {
                start: IDENTIFIER_START,
                continue_with: IDENTIFIER_CONTINUE,
            },
            block_prefixes: &[],
            named_block_keywords: &[],
            method_scopes: &[],
            c_style_functions: false,
            c_fun_type_prefix: false,
            statement_keywords: &[],
            unary_minus_keywords: &["return"],
            boolean_constants: &[],
        }
    }
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

pub(super) const IDENTIFIER_START: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_";
pub(super) const IDENTIFIER_CONTINUE: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_0123456789";

/// The opening and closing text of a string.
///
/// `start` and `end` are matched as written. A `"` string uses the same text
/// for both. A language can list several, and the longest opening text wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StringDelimiter {
    /// Characters that open the string.
    pub start: &'static str,
    /// Characters that close the string.
    pub end: &'static str,
}

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

#[cfg(test)]
mod tests {
    use super::{for_path, BlockCategory, ANSI_C, BLOCK_CATEGORIES, JAVASCRIPT, PYTHON, RUST};

    #[test]
    fn a_category_parses_back_from_its_name() {
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
        assert_eq!(for_path("src/main.js"), Some(&JAVASCRIPT));
        assert_eq!(for_path("src/main.c"), Some(&ANSI_C));
        assert_eq!(for_path("src/fib.h"), Some(&ANSI_C));
        assert_eq!(for_path("notes.txt"), None);
        assert_eq!(for_path("notes.json"), None);
        assert_eq!(for_path("main.rs.bak"), None);
        assert_eq!(for_path("main.js.bak"), None);
        assert_eq!(for_path("main.c.bak"), None);
    }
}
