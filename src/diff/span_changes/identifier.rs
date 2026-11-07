//! Identifying and classifying a renamed name.
//!
//! The two lines are split into names and the text between them. The edit is
//! a rename when those pieces have the same shape, exactly one name differs,
//! and that old name occurs once on the line. A file with no dialect stays a
//! line modification. Comments and strings are still read as ordinary text, so
//! a one-name edit inside a full-line comment stays a rename.

use crate::dialect::{Dialect, IdentifierCharacters};

/// The old and new spelling of one renamed identifier.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Rename {
    pub(crate) oldname: String,
    pub(crate) newname: String,
}

/// The identifier rename on this line, when the whole edit is that one name.
///
/// `None` leaves the edit as a line modification. The old name must occur
/// once: `foo(foo)` changing to `foo(bar)` is not a rename.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn identifier_rename(path: &str, old_line: &str, new_line: &str) -> Option<Rename> {
    let found = super::find_spans(path, old_line, new_line);
    let [span] = found.as_slice() else {
        return None;
    };
    let super::Classification::Rename { oldname, newname } = super::classify(path, span)? else {
        return None;
    };
    Some(Rename { oldname, newname })
}

/// Replaces the one occurrence of `oldname` on this line.
///
/// The name has to occur once as a whole identifier. A missing name, a second
/// occurrence, or a file with no dialect is an error, so apply does not write
/// a different edit.
pub(crate) fn replace_name(
    path: &str,
    line: &str,
    oldname: &str,
    newname: &str,
) -> Result<String, String> {
    let dialect = crate::dialect::for_path(path)
        .ok_or_else(|| format!("{path} has no dialect for an identifier rename"))?;
    let pieces = pieces(dialect, line);
    let mut found = None;
    for piece in &pieces {
        if piece.identifier && piece.text == oldname {
            if found.is_some() {
                return Err(format!("{oldname} does not occur once"));
            }
            found = Some((piece.start, piece.text.len()));
        }
    }
    let Some((start, length)) = found else {
        return Err(format!("{oldname} does not occur once"));
    };
    let mut replaced = String::new();
    replaced.push_str(&line[..start]);
    replaced.push_str(newname);
    replaced.push_str(&line[start + length..]);
    Ok(replaced)
}

/// Replaces every occurrence of `oldname` on this line.
///
/// Zero occurrences leaves the line unchanged. A file with no dialect is an
/// error, so apply does not write a different edit.
pub(crate) fn replace_every(
    path: &str,
    line: &str,
    oldname: &str,
    newname: &str,
) -> Result<String, String> {
    let dialect = crate::dialect::for_path(path)
        .ok_or_else(|| format!("{path} has no dialect for an identifier rename"))?;
    let mut replaced = String::new();
    for piece in pieces(dialect, line) {
        if piece.identifier && piece.text == oldname {
            replaced.push_str(newname);
        } else {
            replaced.push_str(piece.text);
        }
    }
    Ok(replaced)
}

/// How many times `name` occurs as an identifier on this line.
///
/// `None` means the file has no dialect, so the line is not read for names.
pub(crate) fn occurrence_count(path: &str, line: &str, name: &str) -> Option<usize> {
    let dialect = crate::dialect::for_path(path)?;
    Some(count_name(&pieces(dialect, line), name))
}

struct Piece<'a> {
    identifier: bool,
    text: &'a str,
    start: usize,
}

fn count_name(pieces: &[Piece<'_>], name: &str) -> usize {
    pieces
        .iter()
        .filter(|piece| piece.identifier && piece.text == name)
        .count()
}

/// Names and the gaps between them, in order.
///
/// A name starts with a character from `start` and continues with characters
/// from `continue_with`. A digit does not open a name, so `2foo` is a digit
/// followed by the name `foo`.
fn pieces<'a>(dialect: &Dialect, line: &'a str) -> Vec<Piece<'a>> {
    let mut found = Vec::new();
    let characters = &dialect.identifier_characters;
    let mut index = 0;
    while index < line.len() {
        let character = line[index..]
            .chars()
            .next()
            .expect("index is on a character");
        if characters.starts(character) {
            let end = end_of_name(characters, line, index + character.len_utf8());
            found.push(Piece {
                identifier: true,
                text: &line[index..end],
                start: index,
            });
            index = end;
            continue;
        }
        let end = index + character.len_utf8();
        push_other(&mut found, line, index, end);
        index = end;
    }
    found
}

fn end_of_name(characters: &IdentifierCharacters, line: &str, mut index: usize) -> usize {
    while index < line.len() {
        let character = line[index..]
            .chars()
            .next()
            .expect("index is on a character");
        if !characters.continues(character) {
            break;
        }
        index += character.len_utf8();
    }
    index
}

fn push_other<'a>(pieces: &mut Vec<Piece<'a>>, line: &'a str, start: usize, end: usize) {
    if let Some(last) = pieces.last_mut() {
        if !last.identifier && last.start + last.text.len() == start {
            last.text = &line[last.start..end];
            return;
        }
    }
    pieces.push(Piece {
        identifier: false,
        text: &line[start..end],
        start,
    });
}

impl IdentifierCharacters {
    pub(crate) fn starts(&self, character: char) -> bool {
        self.start.contains(character)
    }

    pub(crate) fn continues(&self, character: char) -> bool {
        self.continue_with.contains(character)
    }
}

#[cfg(test)]
mod tests {
    use super::{identifier_rename, replace_name};

    fn renamed(path: &str, old_line: &str, new_line: &str) -> Option<(String, String)> {
        identifier_rename(path, old_line, new_line).map(|rename| (rename.oldname, rename.newname))
    }

    #[test]
    fn one_name_on_the_line_is_a_rename() {
        assert_eq!(
            renamed("src/main.rs", "fn fibonacci(n: u32) {", "fn fib(n: u32) {"),
            Some(("fibonacci".to_string(), "fib".to_string()))
        );
        assert_eq!(
            renamed("src/main.rs", "let count = 1;", "let count2 = 1;"),
            Some(("count".to_string(), "count2".to_string()))
        );
        assert_eq!(
            renamed("src/main.rs", "let value = 2foo;", "let value = 2bar;"),
            Some(("foo".to_string(), "bar".to_string()))
        );
        assert_eq!(
            renamed("calc.py", "def fibonacci(n):", "def fib(n):"),
            Some(("fibonacci".to_string(), "fib".to_string()))
        );
        assert_eq!(
            renamed(
                "src/main.js",
                "function fibonacci(n) {",
                "function fib(n) {"
            ),
            Some(("fibonacci".to_string(), "fib".to_string()))
        );
        assert_eq!(
            renamed("src/main.js", "const $count = 1;", "const $total = 1;"),
            Some(("$count".to_string(), "$total".to_string()))
        );
    }

    #[test]
    fn anything_other_than_one_unique_name_stays_a_modification() {
        assert_eq!(
            renamed("src/main.rs", "foo(foo)", "foo(bar)"),
            None,
            "the old name occurs twice"
        );
        assert_eq!(renamed("src/main.rs", "foo + bar", "fox + baz"), None);
        assert_eq!(renamed("src/main.rs", "foo(1)", "bar(2)"), None);
        assert_eq!(renamed("src/main.rs", "foo", "foo()"), None);
        assert_eq!(renamed("src/main.rs", "foobar", "foo bar"), None);
        assert_eq!(renamed("notes.txt", "fibonacci", "fib"), None);
    }

    #[test]
    fn replace_writes_the_one_occurrence() {
        assert_eq!(
            replace_name("src/main.rs", "fn fibonacci(n: u32) {", "fibonacci", "fib").unwrap(),
            "fn fib(n: u32) {"
        );
        assert!(replace_name("src/main.rs", "foo(foo)", "foo", "bar").is_err());
        assert!(replace_name("src/main.rs", "foobar", "foo", "bar").is_err());
        assert!(replace_name("notes.txt", "fibonacci", "fibonacci", "fib").is_err());
    }
}
