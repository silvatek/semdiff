//! Languages semdiff can read.
//!
//! A changed line asks which characters form a name. A line whose first
//! non-whitespace characters are the line comment is a full-line comment.
//! Block comments and block edges are recorded for a later step. A named
//! block is a function, method, or structure opened by one of the dialect's
//! keywords.

#![cfg_attr(not(test), allow(dead_code))]

/// A language semdiff knows how to read.
///
/// Extensions select its files. The identifier characters say which runs of
/// text are names. The line comment flags a full-line comment. Block comments
/// and block edges are recorded so a later comparison can see the rest. The
/// named-block keywords say when an added run of lines is a new function,
/// method, or structure.
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
    /// Characters that can appear in an identifier.
    pub identifier_characters: IdentifierCharacters,
    /// Words that may precede the keyword opening a named block.
    ///
    /// `pub async fn` has the prefixes `pub` and `async`. A word that is not
    /// in this list stops the search, so `pub(crate) fn` is not recognized.
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

/// The name and category of one named block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedBlock {
    /// The shared category of this block.
    pub category: BlockCategory,
    /// The identifier that names this block.
    pub name: String,
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
/// A block of code starts with `{` and ends with `}`. An identifier starts
/// with a letter or `_` and continues with letters, digits, or `_`. A named
/// block is opened by `fn`, `struct`, `enum`, `union`, `trait`, or `impl`,
/// optionally after `pub`, `async`, `unsafe`, or `const`. Those keywords are
/// function, structure, enumeration, union, interface, and implementation.
/// A method uses `fn` as well, and needs the enclosing type, so a `fn` read
/// from the added lines alone is a function.
pub(crate) static RUST: Dialect = Dialect {
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
    block_prefixes: &["pub", "async", "unsafe", "const"],
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
/// the line. Python does not denote a multi-line comment: a triple-quoted
/// string is a string. A block starts by indenting and ends when the
/// indentation returns. An identifier starts with a letter or `_` and
/// continues with letters, digits, or `_`. Python does not record named
/// blocks yet, so a `def` or `class` stays a line addition.
pub(crate) static PYTHON: Dialect = Dialect {
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
    named_block_keywords: &[],
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

/// Whether `line` introduces this dialect's line comment before any other text.
///
/// Leading whitespace is skipped. The line-comment marker then has to be the
/// next characters. A block comment is not recognized here.
pub(crate) fn introduces_line_comment(dialect: &Dialect, line: &str) -> bool {
    line.trim_start().starts_with(dialect.line_comment)
}

/// Whether `text` is a full-line comment for the file at `path`.
///
/// Every line has to introduce the dialect's line comment. A file with no
/// dialect is not a comment. An empty line is not one either.
pub(crate) fn full_line_comment(path: &str, text: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    if text.is_empty() {
        return false;
    }
    text.split('\n')
        .all(|line| introduces_line_comment(dialect, line))
}

/// Whether `text` is a blank line for the file at `path`.
///
/// A blank line is empty or holds only ASCII spaces and tabs. Rust and Python
/// both ignore that line, so editing one has no functional effect. A file with
/// no dialect is not blank for this purpose. A comment, a carriage return, or
/// any other character keeps the line unflagged.
pub(crate) fn blank_line(path: &str, text: &str) -> bool {
    if for_path(path).is_none() {
        return false;
    }
    text.split('\n').all(only_ascii_blank)
}

/// Whether `line` contains nothing but ASCII spaces and tabs.
fn only_ascii_blank(line: &str) -> bool {
    line.chars().all(is_ascii_blank)
}

/// Whether changing `old_line` into `new_line` touches only an insignificant margin.
///
/// The lines have to match once ASCII spaces and tabs are removed from both
/// ends. A brace dialect treats that indentation as formatting. An indentation
/// dialect treats a leading change as a block boundary, unless both lines are
/// blank or both are full-line comments. Trailing space is insignificant in
/// either dialect. A file with no dialect is not whitespace. A carriage return
/// or any other character stays part of the line.
pub(crate) fn margin_whitespace(path: &str, old_line: &str, new_line: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    if strip_ascii_blank(old_line) != strip_ascii_blank(new_line) {
        return false;
    }
    if leading_ascii_blank(old_line) == leading_ascii_blank(new_line) {
        return true;
    }
    if indent_is_formatting(dialect) {
        return true;
    }
    if only_ascii_blank(old_line) && only_ascii_blank(new_line) {
        return true;
    }
    introduces_line_comment(dialect, old_line) && introduces_line_comment(dialect, new_line)
}

/// The named block added by `text`, when this dialect sees one.
///
/// `text` is one `line-added` record and may hold several lines. Only the
/// first line is read. Leading spaces and tabs are skipped, then any prefix
/// words, then a keyword that opens a named block, then the identifier that
/// names it. The keyword selects the category. A comment, a dialect with no
/// such keywords, and a line that does not open in that shape have no block.
/// The remainder of the line, and any following lines, are kept as the
/// block's text and are not inspected.
pub(crate) fn named_block(path: &str, text: &str) -> Option<NamedBlock> {
    let dialect = for_path(path)?;
    if dialect.named_block_keywords.is_empty() {
        return None;
    }
    let line = text.split('\n').next().unwrap_or("");
    if introduces_line_comment(dialect, line) {
        return None;
    }
    let opened = block_name(dialect, line)?;
    Some(NamedBlock {
        category: opened.category,
        name: opened.name.to_string(),
    })
}

/// The category and name of the block opened by `line`.
fn block_name<'a>(dialect: &Dialect, line: &'a str) -> Option<OpenedBlock<'a>> {
    let mut rest = trim_ascii_blank_start(line);
    loop {
        let (word, after) = next_word(&dialect.identifier_characters, rest)?;
        if let Some(category) = keyword_category(dialect.named_block_keywords, word) {
            let name_at = trim_ascii_blank_start(after);
            if name_at.len() == after.len() {
                return None;
            }
            let (name, _) = next_word(&dialect.identifier_characters, name_at)?;
            return Some(OpenedBlock { category, name });
        }
        if !is_listed(dialect.block_prefixes, word) {
            return None;
        }
        let next = trim_ascii_blank_start(after);
        if next.len() == after.len() {
            return None;
        }
        rest = next;
    }
}

/// A named block found on one line, before the name is copied.
struct OpenedBlock<'a> {
    category: BlockCategory,
    name: &'a str,
}

fn keyword_category(keywords: &[NamedBlockKeyword], word: &str) -> Option<BlockCategory> {
    keywords
        .iter()
        .find(|item| item.keyword == word)
        .map(|item| item.category)
}

fn is_listed(words: &[&str], word: &str) -> bool {
    words.iter().any(|candidate| *candidate == word)
}

/// The next identifier in `line`, and the text that follows it.
fn next_word<'a>(characters: &IdentifierCharacters, line: &'a str) -> Option<(&'a str, &'a str)> {
    let mut chars = line.char_indices();
    let (_, first) = chars.next()?;
    if !characters.starts(first) {
        return None;
    }
    let mut end = first.len_utf8();
    for (index, character) in chars {
        if !characters.continues(character) {
            end = index;
            break;
        }
        end = index + character.len_utf8();
    }
    Some((&line[..end], &line[end..]))
}

fn trim_ascii_blank_start(line: &str) -> &str {
    line.trim_start_matches(is_ascii_blank)
}

/// Whether a block is delimited by tokens, so indentation is formatting.
fn indent_is_formatting(dialect: &Dialect) -> bool {
    matches!(
        (dialect.block_start, dialect.block_end),
        (BlockEdge::Token(_), BlockEdge::Token(_))
    )
}

fn is_ascii_blank(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// `line` without ASCII spaces and tabs at either end.
fn strip_ascii_blank(line: &str) -> &str {
    line.trim_matches(is_ascii_blank)
}

/// The ASCII spaces and tabs at the start of `line`.
fn leading_ascii_blank(line: &str) -> &str {
    let rest = line.trim_start_matches(is_ascii_blank);
    &line[..line.len() - rest.len()]
}

#[cfg(test)]
mod tests {
    use super::{
        blank_line, for_path, full_line_comment, introduces_line_comment, margin_whitespace,
        named_block, BlockCategory, BlockEdge, IdentifierCharacters, NamedBlockKeyword,
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
        assert_eq!(RUST.extensions, [".rs"]);
        assert_eq!(RUST.line_comment, "//");
        let comment = RUST.block_comment.expect("Rust denotes a block comment");
        assert_eq!(comment.start, "/*");
        assert_eq!(comment.end, "*/");
        assert_eq!(RUST.block_start, BlockEdge::Token("{"));
        assert_eq!(RUST.block_end, BlockEdge::Token("}"));
        assert_eq!(RUST.identifier_characters, ascii_identifiers());
        assert_eq!(RUST.block_prefixes, ["pub", "async", "unsafe", "const"]);
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
        assert_eq!(PYTHON.extensions, [".py"]);
        assert_eq!(PYTHON.line_comment, "#");
        assert_eq!(PYTHON.block_comment, None);
        assert_eq!(PYTHON.block_start, BlockEdge::Indentation);
        assert_eq!(PYTHON.block_end, BlockEdge::Indentation);
        assert_eq!(PYTHON.identifier_characters, ascii_identifiers());
        assert_eq!(PYTHON.block_prefixes, [] as [&str; 0]);
        assert_eq!(PYTHON.named_block_keywords, [] as [NamedBlockKeyword; 0]);
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

    #[test]
    fn a_line_comment_is_the_first_non_whitespace() {
        assert!(introduces_line_comment(&RUST, "// note"));
        assert!(introduces_line_comment(&RUST, "    /// doc"));
        assert!(introduces_line_comment(&RUST, "//! inner"));
        assert!(!introduces_line_comment(&RUST, "let x = 1; // note"));
        assert!(!introduces_line_comment(&RUST, "/* block */"));
        assert!(!introduces_line_comment(&RUST, "#[allow(dead_code)]"));
        assert!(!introduces_line_comment(&RUST, ""));
        assert!(!introduces_line_comment(&RUST, "    "));
        assert!(introduces_line_comment(&PYTHON, "# note"));
        assert!(introduces_line_comment(&PYTHON, "  # note"));
        assert!(!introduces_line_comment(&PYTHON, "x = 1  # note"));
        assert!(!introduces_line_comment(&PYTHON, "// not python"));
    }

    #[test]
    fn a_full_line_comment_needs_a_known_dialect() {
        assert!(full_line_comment("src/main.rs", "// note"));
        assert!(full_line_comment("src/main.rs", "// one\n// two"));
        assert!(!full_line_comment("src/main.rs", "// one\nlet x = 1;"));
        assert!(!full_line_comment("src/main.rs", "/* note */"));
        assert!(full_line_comment("calc.py", "# note"));
        assert!(!full_line_comment("notes.txt", "// note"));
        assert!(!full_line_comment("main.rs.bak", "// note"));
        assert!(!full_line_comment("src/main.rs", ""));
    }

    #[test]
    fn a_blank_line_needs_a_known_dialect() {
        assert!(blank_line("src/main.rs", ""));
        assert!(blank_line("src/main.rs", " "));
        assert!(blank_line("src/main.rs", "\t"));
        assert!(blank_line("src/main.rs", " \t "));
        assert!(blank_line("calc.py", ""));
        assert!(blank_line("calc.py", "  "));
        assert!(!blank_line("src/main.rs", "\r"));
        assert!(!blank_line("src/main.rs", " \r"));
        assert!(!blank_line("src/main.rs", "\u{00a0}"));
        assert!(!blank_line("src/main.rs", "let x = 1;"));
        assert!(!blank_line("src/main.rs", "    let x = 1;"));
        assert!(!blank_line("calc.py", "    x = 1"));
        assert!(!blank_line("src/main.rs", "// note"));
        assert!(!blank_line("src/main.rs", "  // note"));
        assert!(!blank_line("calc.py", "# note"));
        assert!(!blank_line("notes.txt", ""));
        assert!(!blank_line("notes.txt", " "));
        assert!(!blank_line("main.rs.bak", " "));
    }

    #[test]
    fn margin_whitespace_follows_the_block_edge() {
        assert!(margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "    let value = 1;"
        ));
        assert!(margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "let value = 1; "
        ));
        assert!(margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "\tlet value = 1;\t"
        ));
        assert!(margin_whitespace("src/main.rs", "// note", "    // note"));
        assert!(margin_whitespace("calc.py", "x = 1", "x = 1 "));
        assert!(margin_whitespace("calc.py", "x = 1", "x = 1\t"));
        assert!(margin_whitespace("calc.py", "# note", "  # note"));
        assert!(margin_whitespace("calc.py", "# note", "# note "));
        assert!(margin_whitespace("src/main.rs", "    ", "\t"));
        assert!(margin_whitespace("calc.py", "", "  "));
        assert!(!margin_whitespace("calc.py", "x = 1", "    x = 1"));
        assert!(!margin_whitespace("calc.py", "x = 1", " x = 1 "));
        assert!(!margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "let  value = 1;"
        ));
        assert!(!margin_whitespace(
            "src/main.rs",
            "let value = 1;",
            "let value = 1;\r"
        ));
        assert!(!margin_whitespace("src/main.rs", "    ", "let value = 1;"));
        assert!(!margin_whitespace("notes.txt", "hello", "hello "));
        assert!(!margin_whitespace("notes.txt", "hello", "  hello"));
        assert!(!margin_whitespace(
            "main.rs.bak",
            "let value = 1;",
            "  let value = 1;"
        ));
    }

    fn opened(path: &str, text: &str) -> Option<(BlockCategory, String)> {
        named_block(path, text).map(|block| (block.category, block.name))
    }

    #[test]
    fn a_rust_addition_names_the_block_it_opens() {
        assert_eq!(
            opened("src/main.rs", "fn fib() {}"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "    fn fib() {"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub async fn run() {\n    work();\n}"),
            Some((BlockCategory::Function, "run".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "const fn ready() {}"),
            Some((BlockCategory::Function, "ready".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "struct Point {"),
            Some((BlockCategory::Structure, "Point".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "struct Unit;"),
            Some((BlockCategory::Structure, "Unit".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "unsafe impl Foo {"),
            Some((BlockCategory::Implementation, "Foo".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub enum Color {"),
            Some((BlockCategory::Enumeration, "Color".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "union Raw {"),
            Some((BlockCategory::Union, "Raw".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "trait Draw {"),
            Some((BlockCategory::Interface, "Draw".to_string()))
        );
    }

    #[test]
    fn an_addition_that_does_not_open_a_named_block_stays_unnamed() {
        assert_eq!(named_block("src/main.rs", "let value = 1;"), None);
        assert_eq!(named_block("src/main.rs", "const VALUE: i32 = 1;"), None);
        assert_eq!(
            named_block("src/main.rs", "pub(crate) fn hidden() {}"),
            None
        );
        assert_eq!(
            named_block("src/main.rs", "#[derive(Debug)]\nstruct Foo {}"),
            None
        );
        assert_eq!(named_block("src/main.rs", "impl<T> Foo {"), None);
        assert_eq!(named_block("src/main.rs", "// fn fake() {}"), None);
        assert_eq!(named_block("src/main.rs", "fn"), None);
        assert_eq!(named_block("src/main.rs", ""), None);
        assert_eq!(named_block("src/main.rs", "\nfn fib() {}"), None);
        assert_eq!(named_block("calc.py", "def fib():\n    return 1"), None);
        assert_eq!(named_block("calc.py", "class Point:"), None);
        assert_eq!(named_block("notes.txt", "fn fib() {}"), None);
    }
}
