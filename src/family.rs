//! Reading named blocks in source, including where one opens, closes, and
//! continues.
//!
//! Brace-delimited languages share the C-family reader. Python has its own.
//! A function or procedure inside one of the dialect's method scopes is a method.

pub(crate) mod c_family;
pub(crate) mod python;

use crate::dialect::{for_path, BlockCategory, Dialect, Family, IdentifierCharacters};

/// The name and category of one named block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedBlock {
    /// The shared category of this block.
    pub category: BlockCategory,
    /// The identifier that names this block.
    pub name: String,
    /// The enclosing type, when this block is a method and that type has a name.
    ///
    /// `Count` in `impl Count` is written on the method's scope as
    /// `src/main.rs[Count]`.
    pub enclosing: Option<String>,
}

/// How an open block affects a callable inside it.
pub(super) enum BlockRole {
    /// A function or procedure. An inner callable stays a function.
    Callable,
    /// A type block. The name is the identifier after its keyword, when it has one.
    Method(Option<String>),
}

/// The type block around a method.
///
/// `name` is the enclosing block's identifier. A method's scope is the file
/// and that name, as `src/main.rs[Count]`. The name is absent when the header
/// has no identifier.
pub(super) struct EnclosingBlock {
    pub name: Option<String>,
}

/// The longest listed qualifier that `rest` starts with.
///
/// The match ends on a token boundary, so `pub` matches `pub fn` and does not
/// match `public`. A longer entry wins, so `pub(crate)` is taken ahead of
/// `pub`. The qualifier is the text to skip; the caller reads what follows.
pub(super) fn matching_qualifier<'a>(
    characters: &IdentifierCharacters,
    qualifiers: &[&str],
    rest: &'a str,
) -> Option<&'a str> {
    qualifiers
        .iter()
        .filter(|qualifier| {
            let Some(after) = rest.strip_prefix(*qualifier) else {
                return false;
            };
            match after.chars().next() {
                Some(character) => !characters.continues(character),
                None => true,
            }
        })
        .max_by_key(|qualifier| qualifier.len())
        .map(|qualifier| &rest[..qualifier.len()])
}

/// Whether `following` is still inside the block opened by `addition`.
///
/// Blank lines are not an answer: the caller skips them and asks about the
/// next line. The C-family reader has nothing to add here, because an unclosed
/// brace block is already not a named block. Python uses indentation, so a
/// further-indented line is still in the block.
pub(crate) fn block_resumes(path: &str, addition: &str, following: &str) -> bool {
    let Some(dialect) = for_path(path) else {
        return false;
    };
    match dialect.family {
        Family::CFamily => false,
        Family::Python => python::block_resumes(addition, following),
    }
}

/// Whether `word` is a callable keyword, such as `fn` or `def`.
///
/// A function or a procedure can become a method when it sits in a type's
/// block. The keyword's own category stays what the dialect configured.
pub(super) fn keyword_is_callable(dialect: &Dialect, word: &str) -> bool {
    dialect.named_block_keywords.iter().any(|item| {
        item.keyword == word
            && matches!(
                item.category,
                BlockCategory::Function | BlockCategory::Procedure
            )
    })
}

/// Whether `word` opens a block in which a callable is a method.
pub(super) fn keyword_is_method_scope(dialect: &Dialect, word: &str) -> bool {
    dialect.method_scopes.iter().any(|scope| *scope == word)
}

/// The identifier that follows a keyword, when whitespace separates them.
///
/// `impl Count` yields `Count`. `implCount` and `impl {` yield nothing. The
/// name stops at the first character that cannot continue an identifier, so
/// `class Count(Bar)` yields `Count`.
pub(super) fn identifier_after<'a>(
    characters: &IdentifierCharacters,
    after: &'a str,
) -> Option<&'a str> {
    let name_at = after.trim_start_matches(|character: char| character == ' ' || character == '\t');
    if name_at.len() == after.len() {
        return None;
    }
    let mut chars = name_at.char_indices();
    let (_, first) = chars.next()?;
    if !characters.starts(first) {
        return None;
    }
    let mut end = first.len_utf8();
    for (index, character) in chars {
        if !characters.continues(character) {
            break;
        }
        end = index + character.len_utf8();
    }
    Some(&name_at[..end])
}

/// The named block in `text`, when the dialect's reader sees one.
///
/// `text` is one line record and may hold several lines. Each reader uses its
/// dialect's keywords, and the text has to be that one complete block. The
/// block is read as if it stood at the top of the file.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn named_block(path: &str, text: &str) -> Option<NamedBlock> {
    named_block_in(path, text, "")
}

/// The named block in `text`, read with the file above it.
///
/// `before` is the file above the block, without the block itself. An
/// addition uses the new file. A removal uses the old file. A function or
/// procedure becomes a method when the innermost open block that is a
/// callable or a method scope is one of the dialect's method scopes. The
/// method records that block's name. An empty `before` means the block is at
/// the top of the file.
pub(crate) fn named_block_in(path: &str, text: &str, before: &str) -> Option<NamedBlock> {
    let dialect = for_path(path)?;
    let enclosing = enclosing_block(dialect, before, text);
    let mut block = match dialect.family {
        Family::CFamily => c_family::named_block(dialect, text, enclosing.as_ref())?,
        Family::Python => python::named_block(dialect, text)?,
    };
    if can_be_a_method(block.category) {
        if let Some(enclosing) = enclosing {
            block.category = BlockCategory::Method;
            block.enclosing = enclosing.name;
        }
    }
    Some(block)
}

/// Whether a function or procedure inside a type's block is recorded as a method.
fn can_be_a_method(category: BlockCategory) -> bool {
    matches!(category, BlockCategory::Function | BlockCategory::Procedure)
}

/// The method scope around the addition, when `before` leaves it inside one.
fn enclosing_block(dialect: &Dialect, before: &str, addition: &str) -> Option<EnclosingBlock> {
    match dialect.family {
        Family::CFamily => c_family::enclosing_block(dialect, before),
        Family::Python => python::enclosing_block(dialect, before, addition),
    }
}

#[cfg(test)]
mod tests {
    use super::{named_block, named_block_in};
    use crate::dialect::BlockCategory;

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
            opened(
                "src/main.rs",
                "    fn fib() {\n        let value = 1;\n    }"
            ),
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
            opened("src/main.rs", "pub(crate) fn hidden() {}"),
            Some((BlockCategory::Function, "hidden".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub(super) fn hidden() {}"),
            Some((BlockCategory::Function, "hidden".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub(self) async fn hidden() {}"),
            Some((BlockCategory::Function, "hidden".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "struct Point {\n    x: i32,\n}"),
            Some((BlockCategory::Structure, "Point".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "unsafe impl Foo {\n    fn ready() {}\n}"),
            Some((BlockCategory::Implementation, "Foo".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "pub enum Color {\n    Red,\n}"),
            Some((BlockCategory::Enumeration, "Color".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "union Raw {\n    n: u32,\n}"),
            Some((BlockCategory::Union, "Raw".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "trait Draw {\n    fn draw();\n}"),
            Some((BlockCategory::Interface, "Draw".to_string()))
        );
        assert_eq!(
            opened(
                "src/main.rs",
                "fn nested() {\n    if ready {\n        work();\n    }\n}"
            ),
            Some((BlockCategory::Function, "nested".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "fn noted() { // }\n}"),
            Some((BlockCategory::Function, "noted".to_string()))
        );
        assert_eq!(
            opened("src/main.rs", "fn noted() { /* } */ }"),
            Some((BlockCategory::Function, "noted".to_string()))
        );
        assert_eq!(
            opened("calc.py", "def fib():\n    return 1"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened(
                "calc.py",
                "def fib():\n    if n:\n        return 1\n    return 0"
            ),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("calc.py", "def fib():\n    return 1\n\n    return 2"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened("calc.py", "class Point:\n    x = 1"),
            Some((BlockCategory::Class, "Point".to_string()))
        );
    }

    #[test]
    fn an_addition_has_to_be_one_complete_block() {
        assert_eq!(named_block("src/main.rs", "fn fib() {"), None);
        assert_eq!(
            named_block("src/main.rs", "fn fib() {\n    let value = 1;"),
            None
        );
        assert_eq!(named_block("src/main.rs", "fn fib() {\n    // }\n"), None);
        assert_eq!(named_block("src/main.rs", "struct Point {"), None);
        assert_eq!(named_block("src/main.rs", "struct Unit;"), None);
        assert_eq!(named_block("src/main.rs", "fn one() {}\nfn two() {}"), None);
        assert_eq!(named_block("calc.py", "def fib():"), None);
        assert_eq!(named_block("calc.py", "class Point:"), None);
        assert_eq!(
            named_block(
                "calc.py",
                "def one():\n    return 1\ndef two():\n    return 2"
            ),
            None
        );
    }

    #[test]
    fn an_addition_that_does_not_open_a_named_block_stays_unnamed() {
        assert_eq!(named_block("src/main.rs", "let value = 1;"), None);
        assert_eq!(named_block("src/main.rs", "const VALUE: i32 = 1;"), None);
        assert_eq!(named_block("src/main.rs", "public fn hidden() {}"), None);
        assert_eq!(
            named_block("src/main.rs", "pub(in crate::util) fn hidden() {}"),
            None
        );
        assert_eq!(named_block("src/main.rs", "pub(crate)fn hidden() {}"), None);
        assert_eq!(
            named_block("src/main.rs", "#[derive(Debug)]\nstruct Foo {}"),
            None
        );
        assert_eq!(named_block("src/main.rs", "impl<T> Foo {"), None);
        assert_eq!(named_block("src/main.rs", "// fn fake() {}"), None);
        assert_eq!(named_block("src/main.rs", "fn"), None);
        assert_eq!(named_block("src/main.rs", ""), None);
        assert_eq!(named_block("src/main.rs", "\nfn fib() {}"), None);
        assert_eq!(named_block("calc.py", "@decorator\ndef fib():"), None);
        assert_eq!(named_block("notes.txt", "fn fib() {}"), None);
        assert_eq!(named_block("src/main.rs", "foo() {}"), None);
        assert_eq!(
            named_block("src/main.js", "function* fib() {\n    return 1;\n}"),
            None
        );
        assert_eq!(
            named_block("src/main.js", "// note\nfunction fib() {}"),
            None
        );
        assert_eq!(
            named_block("src/main.c", "step() {\n    return 1;\n}"),
            None
        );
        assert_eq!(
            named_block("src/main.c", "if (n) {\n    return 1;\n}"),
            None
        );
        assert_eq!(
            named_block("src/main.js", "if (ready) {\n    work();\n}"),
            None
        );
        assert_eq!(
            named_block("src/main.js", "catch (error) {\n    work();\n}"),
            None
        );
    }

    #[test]
    fn a_c_style_function_follows_the_dialect() {
        assert_eq!(
            opened("src/main.c", "int shifted(int n) {\n    return n + 1;\n}"),
            Some((BlockCategory::Function, "shifted".to_string()))
        );
        assert_eq!(
            opened("src/main.c", "static int step(int n) {\n    return n;\n}"),
            Some((BlockCategory::Function, "step".to_string()))
        );
        assert_eq!(
            opened("src/main.c", "int *step(void) {\n    return 1;\n}"),
            Some((BlockCategory::Function, "step".to_string()))
        );
        assert_eq!(
            opened("src/main.js", "step() {\n    return 1;\n}"),
            Some((BlockCategory::Function, "step".to_string()))
        );
        assert_eq!(
            opened_in(
                "src/main.js",
                "    step() {\n        return this.by;\n    }",
                "class Count {"
            ),
            Some((
                BlockCategory::Method,
                "step".to_string(),
                Some("Count".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "src/main.js",
                "    step() {\n        return 1;\n    }",
                "function outer() {"
            ),
            Some((BlockCategory::Function, "step".to_string(), None))
        );
    }

    #[test]
    fn a_javascript_addition_names_the_block_it_opens() {
        assert_eq!(
            opened("src/main.js", "function fib() {\n    return 1;\n}"),
            Some((BlockCategory::Function, "fib".to_string()))
        );
        assert_eq!(
            opened(
                "src/main.js",
                "export async function run() {\n    work();\n}"
            ),
            Some((BlockCategory::Function, "run".to_string()))
        );
        assert_eq!(
            opened("src/main.js", "export default class Point {\n    x = 1;\n}"),
            Some((BlockCategory::Class, "Point".to_string()))
        );
        assert_eq!(
            opened("src/main.js", "function $fib() {}"),
            Some((BlockCategory::Function, "$fib".to_string()))
        );
        assert_eq!(
            opened_in(
                "src/main.js",
                "    function added() {\n        return 1;\n    }",
                "class Count {"
            ),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Count".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "src/main.js",
                "    step() {\n        return this.by;\n    }",
                "class Count {"
            ),
            Some((
                BlockCategory::Method,
                "step".to_string(),
                Some("Count".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "src/main.js",
                "    function added() {\n        return 1;\n    }",
                "function outer() {"
            ),
            Some((BlockCategory::Function, "added".to_string(), None))
        );
    }

    #[test]
    fn a_callable_inside_a_type_is_a_method() {
        assert_eq!(
            opened_in("src/main.rs", "    fn added() {}", "impl Foo {"),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Foo".to_string())
            ))
        );
        assert_eq!(
            opened_in("src/main.rs", "    fn added() {}", "impl Foo for Bar {"),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Foo".to_string())
            ))
        );
        assert_eq!(
            opened_in("src/main.rs", "    fn draw() {}", "pub unsafe trait Draw {"),
            Some((
                BlockCategory::Method,
                "draw".to_string(),
                Some("Draw".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "src/main.rs",
                "    fn added() {}",
                "impl Foo {\n    fn method() {\n        if ready {"
            ),
            Some((BlockCategory::Function, "added".to_string(), None))
        );
        assert_eq!(
            opened_in(
                "src/main.rs",
                "    fn added() {}",
                "fn outer() -> impl Bar {"
            ),
            Some((BlockCategory::Function, "added".to_string(), None))
        );
        assert_eq!(
            opened_in(
                "src/main.rs",
                "    struct Point {\n        x: i32,\n    }",
                "impl Foo {"
            ),
            Some((BlockCategory::Structure, "Point".to_string(), None))
        );
        assert_eq!(
            opened_in("src/main.rs", "    fn added() {}", "impl Foo { /* } */"),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Foo".to_string())
            ))
        );
        assert_eq!(
            opened_in("src/main.rs", "    fn added() {}", "impl {"),
            Some((BlockCategory::Method, "added".to_string(), None))
        );
        assert_eq!(
            opened_in(
                "calc.py",
                "    def added(self):\n        return 1",
                "class Foo:"
            ),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Foo".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "calc.py",
                "    def added(self):\n        return 1",
                "class Count(Bar):"
            ),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Count".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "calc.py",
                "        def added(self):\n            return 1",
                "class Foo:\n    if True:"
            ),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Foo".to_string())
            ))
        );
        assert_eq!(
            opened_in(
                "calc.py",
                "        def added():\n            return 1",
                "class Foo:\n    def method(self):"
            ),
            Some((BlockCategory::Function, "added".to_string(), None))
        );
        assert_eq!(
            opened_in(
                "calc.py",
                "        def added(self):\n            return 1",
                "def outer():\n    class Foo:"
            ),
            Some((
                BlockCategory::Method,
                "added".to_string(),
                Some("Foo".to_string())
            ))
        );
    }

    fn opened_in(
        path: &str,
        text: &str,
        before: &str,
    ) -> Option<(BlockCategory, String, Option<String>)> {
        named_block_in(path, text, before)
            .map(|block| (block.category, block.name, block.enclosing))
    }
}
