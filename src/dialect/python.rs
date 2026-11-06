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
};

#[cfg(test)]
mod tests {
    use super::PYTHON;
    use crate::dialect::{
        BlockCategory, BlockEdge, Family, IdentifierCharacters, NamedBlockKeyword,
    };

    fn ascii_identifiers() -> IdentifierCharacters {
        IdentifierCharacters {
            start: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_",
            continue_with: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_0123456789",
        }
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
        assert_eq!(PYTHON.method_scopes, ["class"]);
    }
}
