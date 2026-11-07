//! An integer whose text changed on one line.
//!
//! The edit is a constant change when the two lines differ only inside one
//! integer. An integer is a maximal ASCII digit sequence. A leading `-` is
//! part of it when that minus starts the number: nothing before it produces
//! a value, so the start of the line, an operator, or a word such as
//! `return` all qualify. A minus after a value is subtraction and stays on
//! the line, as in `count-1` or `items[i]-1`. A `--` glued to the digits is
//! an operator. The line is not a line comment, the digits are not inside a
//! string or a comment, and they do not continue an identifier. A file with
//! no dialect is not read for constants.
//!
//! A boolean constant is a spelling the dialect lists. The edit is a constant
//! change when the lines differ only by one such spelling changing to another,
//! and that spelling is a whole name. A spelling inside a string or a comment
//! stays an identifier rename. ANSI C lists none. `span::classify` makes that
//! decision from the one changed span.

use crate::dialect::Dialect;

/// The old integer, the new integer, and which digit sequence on the old line changed.
///
/// `occurrence` counts maximal digit sequences on the old line, starting at 1.
/// A sequence inside a string or an identifier still counts, so the index
/// stays put when those runs are not the integer that changed.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct IntegerConstant {
    pub(crate) old_value: String,
    pub(crate) new_value: String,
    pub(crate) occurrence: usize,
}

/// One integer on a line.
///
/// `start` includes a unary minus when that minus belongs to the number.
pub(crate) struct IntegerToken {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// The integer constant changed between these two lines, when that is the whole edit.
///
/// `None` leaves the edit as a line modification. The changed span has to be
/// one integer outside a string and a comment. Shared digits at either end of
/// that integer belong to it, so `12` becoming `13` is one integer.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn integer_constant(
    path: &str,
    old_line: &str,
    new_line: &str,
) -> Option<IntegerConstant> {
    let found = crate::span::find_spans(path, old_line, new_line);
    let [span] = found.as_slice() else {
        return None;
    };
    let crate::span::Classification::Constant {
        old_value,
        new_value,
        occurrence,
    } = crate::span::classify(path, span)?
    else {
        return None;
    };
    if !is_integer(&old_value) {
        return None;
    }
    Some(IntegerConstant {
        old_value,
        new_value,
        occurrence,
    })
}

/// Replaces one integer on `line`.
///
/// `occurrence` is the 1-based index among maximal digit sequences. The text
/// of that integer, including a unary minus when the dialect counts it, has
/// to be `old_value`. A missing sequence is an error, so apply does not write
/// a different edit.
pub(crate) fn replace_integer(
    path: &str,
    line: &str,
    old_value: &str,
    new_value: &str,
    occurrence: usize,
) -> Result<String, String> {
    let Some(dialect) = crate::dialect::for_path(path) else {
        return Err(format!("{path} has no dialect"));
    };
    let Some((digits, end)) = digit_runs(line).get(occurrence.wrapping_sub(1)).copied() else {
        return Err(format!("integer {old_value} does not occur"));
    };
    let start = if occurrence == 0 || continues_identifier(dialect, line, digits) {
        digits
    } else {
        signed_start(dialect, line, digits)
    };
    if occurrence == 0 || &line[start..end] != old_value {
        return Err(format!("integer {old_value} does not occur"));
    }
    let mut replaced = String::new();
    replaced.push_str(&line[..start]);
    replaced.push_str(new_value);
    replaced.push_str(&line[end..]);
    Ok(replaced)
}

/// Whether `text` is an integer constant, optionally signed.
///
/// A sign is one leading `-`. Spaces or tabs may sit between that minus and
/// the digits, as they do in `return - 1`. The digits are a non-empty ASCII
/// sequence. `+1`, `--1`, and a trailing space are not integers.
pub(crate) fn is_integer(text: &str) -> bool {
    let digits = match text.strip_prefix('-') {
        Some(rest) => rest.trim_start_matches([' ', '\t']),
        None => text,
    };
    !digits.is_empty() && digits.chars().all(|character| character.is_ascii_digit())
}

/// The old spelling, the new spelling, and which boolean on the old line changed.
///
/// `occurrence` counts spellings of the dialect's boolean constants on the old
/// line, starting at 1. A spelling inside a string still counts, so the index
/// stays put when that one is not the constant that changed.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct BooleanConstant {
    pub(crate) old_value: String,
    pub(crate) new_value: String,
    pub(crate) occurrence: usize,
}

/// The boolean constant changed between these two lines, when that is the whole edit.
///
/// `None` leaves the edit to the other classifiers. Both sides of the changed
/// span have to be spellings the dialect lists, outside a string and a
/// comment. A file with no dialect, and a dialect that lists none, is not read
/// for boolean constants.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn boolean_constant(
    path: &str,
    old_line: &str,
    new_line: &str,
) -> Option<BooleanConstant> {
    let found = crate::span::find_spans(path, old_line, new_line);
    let [span] = found.as_slice() else {
        return None;
    };
    let crate::span::Classification::Constant {
        old_value,
        new_value,
        occurrence,
    } = crate::span::classify(path, span)?
    else {
        return None;
    };
    if !is_boolean(path, &old_value) {
        return None;
    }
    Some(BooleanConstant {
        old_value,
        new_value,
        occurrence,
    })
}

/// Replaces one boolean constant on `line`.
///
/// `occurrence` is the 1-based index among the dialect's boolean spellings.
/// The text of that spelling has to be `old_value`. A missing spelling is an
/// error, so apply does not write a different edit.
pub(crate) fn replace_boolean(
    path: &str,
    line: &str,
    old_value: &str,
    new_value: &str,
    occurrence: usize,
) -> Result<String, String> {
    let Some(dialect) = crate::dialect::for_path(path) else {
        return Err(format!("{path} has no dialect"));
    };
    let Some((start, end)) = boolean_tokens(dialect, line)
        .get(occurrence.wrapping_sub(1))
        .copied()
    else {
        return Err(format!("boolean {old_value} does not occur"));
    };
    if occurrence == 0 || &line[start..end] != old_value {
        return Err(format!("boolean {old_value} does not occur"));
    }
    let mut replaced = String::new();
    replaced.push_str(&line[..start]);
    replaced.push_str(new_value);
    replaced.push_str(&line[end..]);
    Ok(replaced)
}

/// Whether `text` is a boolean spelling for the file at `path`.
///
/// The dialect lists the spellings. A file with no dialect, and a dialect
/// that lists none, has no boolean constants.
pub(crate) fn is_boolean(path: &str, text: &str) -> bool {
    let Some(dialect) = crate::dialect::for_path(path) else {
        return false;
    };
    dialect
        .boolean_constants
        .iter()
        .any(|spelling| *spelling == text)
}

/// Boolean spellings on `line`, as byte offsets of the whole name.
///
/// A spelling is a whole identifier. `true_flag` is a name, not the boolean
/// `true`. A spelling inside a string is still listed, so `occurrence` stays
/// put.
fn boolean_tokens(dialect: &Dialect, line: &str) -> Vec<(usize, usize)> {
    let characters = &dialect.identifier_characters;
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < line.len() {
        let Some(character) = line[index..].chars().next() else {
            break;
        };
        if characters.starts(character) {
            let end = end_of_name(characters, line, index + character.len_utf8());
            let text = &line[index..end];
            if dialect
                .boolean_constants
                .iter()
                .any(|spelling| *spelling == text)
            {
                tokens.push((index, end));
            }
            index = end;
            continue;
        }
        index += character.len_utf8();
    }
    tokens
}

/// The byte index just after the identifier that starts before `index`.
fn end_of_name(
    characters: &crate::dialect::IdentifierCharacters,
    line: &str,
    mut index: usize,
) -> usize {
    while index < line.len() {
        let Some(character) = line[index..].chars().next() else {
            break;
        };
        if !characters.continues(character) {
            break;
        }
        index += character.len_utf8();
    }
    index
}

/// The 0-based index of the digit run that starts at `digits`.
///
/// Runs inside names and strings count, so an integer later on the line keeps
/// its index when those runs are not the one that changed.
pub(crate) fn digit_run_index(line: &str, digits: usize) -> Option<usize> {
    digit_runs(line)
        .iter()
        .position(|(start, _)| *start == digits)
}

/// Integers on `line`, in order.
///
/// A digit sequence that continues an identifier is not an integer. It stays
/// in the text between integers, and it still counts toward `occurrence`.
pub(crate) fn integer_tokens(dialect: &Dialect, line: &str) -> Vec<IntegerToken> {
    digit_runs(line)
        .into_iter()
        .filter(|(digits, _)| !continues_identifier(dialect, line, *digits))
        .map(|(digits, end)| IntegerToken {
            start: signed_start(dialect, line, digits),
            end,
        })
        .collect()
}

/// The byte index of a unary `-` belonging to the digits at `digits`, or `digits` itself.
///
/// The minus has to be the nearest non-space character before the digits.
/// `--` immediately before those digits is an operator, so the digits stand alone.
fn signed_start(dialect: &Dialect, line: &str, digits: usize) -> usize {
    let mut cursor = digits;
    while cursor > 0 {
        let Some(character) = line[..cursor].chars().next_back() else {
            break;
        };
        if character != ' ' && character != '\t' {
            break;
        }
        cursor -= character.len_utf8();
    }
    if cursor == 0 || !line[..cursor].ends_with('-') {
        return digits;
    }
    let minus_at = cursor - '-'.len_utf8();
    if minus_at > 0 && line.as_bytes()[minus_at - 1] == b'-' && minus_at + 1 == digits {
        return digits;
    }
    if minus_is_unary(dialect, line, minus_at) {
        minus_at
    } else {
        digits
    }
}

/// Whether the `-` at `minus_at` starts a number.
///
/// A value ends at an identifier, a digit, a closing bracket, or a quote.
/// `true-1` is subtraction. A listed word such as `return` or `not` does not
/// produce a value, so `return -1` is the integer. `>` is an operator, so
/// `a > -1` includes the sign.
fn minus_is_unary(dialect: &Dialect, line: &str, minus_at: usize) -> bool {
    let Some((character, character_at)) = previous_character(line, minus_at) else {
        return true;
    };
    if matches!(character, ')' | ']' | '}' | '\'' | '"' | '`') {
        return false;
    }
    if !dialect.identifier_characters.continues(character) {
        return true;
    }
    let word_end = character_at + character.len_utf8();
    let Some(word) = word_ending_at(dialect, line, word_end) else {
        return false;
    };
    dialect
        .unary_minus_keywords
        .iter()
        .any(|keyword| *keyword == word)
}

/// The nearest character before `index` that is not a space or a tab.
fn previous_character(line: &str, index: usize) -> Option<(char, usize)> {
    let mut cursor = index;
    loop {
        if cursor == 0 {
            return None;
        }
        let character = line[..cursor].chars().next_back()?;
        let start = cursor - character.len_utf8();
        if character == ' ' || character == '\t' {
            cursor = start;
            continue;
        }
        return Some((character, start));
    }
}

/// The identifier ending at `end`, when its first character can open a name.
fn word_ending_at<'a>(dialect: &Dialect, line: &'a str, end: usize) -> Option<&'a str> {
    let mut start = end;
    while start > 0 {
        let character = line[..start].chars().next_back()?;
        if !dialect.identifier_characters.continues(character) {
            break;
        }
        start -= character.len_utf8();
    }
    if start == end {
        return None;
    }
    let word = &line[start..end];
    let first = word.chars().next()?;
    if !dialect.identifier_characters.starts(first) {
        return None;
    }
    Some(word)
}

/// Whether the character before `start` continues an identifier.
///
/// A digit sequence there is the tail of a name, such as the `1` in `count1`.
fn continues_identifier(dialect: &Dialect, line: &str, start: usize) -> bool {
    let Some(before) = line[..start].chars().next_back() else {
        return false;
    };
    dialect.identifier_characters.continues(before)
}

fn expand_right(line: &str, mut index: usize) -> usize {
    while index < line.len() {
        let Some(character) = line[index..].chars().next() else {
            break;
        };
        if !character.is_ascii_digit() {
            break;
        }
        index += character.len_utf8();
    }
    index
}

/// Maximal ASCII digit sequences on `line`, as byte offsets.
fn digit_runs(line: &str) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut index = 0;
    while index < line.len() {
        let Some(character) = line[index..].chars().next() else {
            break;
        };
        if character.is_ascii_digit() {
            let end = expand_right(line, index);
            runs.push((index, end));
            index = end;
            continue;
        }
        index += character.len_utf8();
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::{
        boolean_constant, integer_constant, is_boolean, is_integer, replace_boolean,
        replace_integer,
    };

    fn changed(path: &str, old_line: &str, new_line: &str) -> Option<(String, String, usize)> {
        integer_constant(path, old_line, new_line)
            .map(|constant| (constant.old_value, constant.new_value, constant.occurrence))
    }

    #[test]
    fn one_integer_on_the_line_is_a_constant() {
        assert_eq!(
            changed("src/main.rs", "let n = 1;", "let n = 42;"),
            Some(("1".to_string(), "42".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 12;", "let n = 13;"),
            Some(("12".to_string(), "13".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 1;", "let n = 10;"),
            Some(("1".to_string(), "10".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 1u32;", "let n = 2u32;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed(
                "src/main.rs",
                "let n = 1; let m = 1;",
                "let n = 1; let m = 2;"
            ),
            Some(("1".to_string(), "2".to_string(), 2))
        );
        assert_eq!(
            changed("src/main.rs", "let n: u32 = 1;", "let n: u32 = 2;"),
            Some(("1".to_string(), "2".to_string(), 2))
        );
        assert_eq!(
            changed("src/main.c", "int n = 8;", "int n = 9;"),
            Some(("8".to_string(), "9".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.js", "const n = 1;", "const n = 2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("calc.py", "n = 1", "n = 2"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 2foo;", "let n = 3foo;"),
            Some(("2".to_string(), "3".to_string(), 1))
        );
    }

    #[test]
    fn a_unary_minus_is_part_of_the_integer() {
        assert_eq!(
            changed("src/main.rs", "let n = -1;", "let n = -2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = (-1);", "let n = (-2);"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n=-1;", "let n=-2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = -1;", "let n = 1;"),
            Some(("-1".to_string(), "1".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 1;", "let n = -1;"),
            Some(("1".to_string(), "-1".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "return -1;", "return -2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "return-1;", "return-2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "return - 1;", "return - 2;"),
            Some(("- 1".to_string(), "- 2".to_string(), 1))
        );
        assert_eq!(
            changed("calc.py", "return not -1", "return not -2"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.js", "return -1;", "return -2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.c", "return -1;", "return -2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.c", "return sizeof-1;", "return sizeof-2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 1..-1;", "let n = 1..-2;"),
            Some(("-1".to_string(), "-2".to_string(), 2))
        );
        assert_eq!(
            changed("src/main.rs", "let n = a > -1;", "let n = a > -2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = {1}-1;", "let n = {1}-2;"),
            Some(("1".to_string(), "2".to_string(), 2))
        );
        assert!(is_integer("-10"));
        assert!(is_integer("- 1"));
        assert!(!is_integer("--1"));
        assert!(!is_integer("-"));
        assert!(!is_integer("+1"));
    }

    #[test]
    fn a_minus_after_a_value_stays_subtraction() {
        assert_eq!(
            changed("src/main.rs", "let n = a-1;", "let n = a-2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = foo-1;", "let n = foo-2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = 1-1;", "let n = 1-2;"),
            Some(("1".to_string(), "2".to_string(), 2))
        );
        assert_eq!(
            changed("src/main.rs", "let n = (a)-1;", "let n = (a)-2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = items[i]-1;", "let n = items[i]-2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = --1;", "let n = --2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = - -1;", "let n = - -2;"),
            Some(("-1".to_string(), "-2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.c", "int n = p->1;", "int n = p->2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
        assert_eq!(
            changed("src/main.rs", "let n = true-1;", "let n = true-2;"),
            Some(("1".to_string(), "2".to_string(), 1))
        );
    }

    #[test]
    fn a_string_a_comment_or_a_name_stays_a_modification() {
        assert_eq!(
            changed("src/main.rs", "let s = \"1\";", "let s = \"2\";"),
            None
        );
        assert_eq!(
            changed("src/main.rs", "let s = \"-1\";", "let s = \"-2\";"),
            None
        );
        assert_eq!(
            changed(
                "src/main.rs",
                "let s = \"1\"; let n = 1;",
                "let s = \"1\"; let n = 2;"
            ),
            Some(("1".to_string(), "2".to_string(), 2))
        );
        assert_eq!(
            changed("src/main.js", "const s = '1';", "const s = '2';"),
            None
        );
        assert_eq!(changed("calc.py", "s = '''1'''", "s = '''2'''"), None);
        assert_eq!(changed("src/main.rs", "// 1", "// 2"), None);
        assert_eq!(changed("src/main.rs", "// -1", "// -2"), None);
        assert_eq!(changed("calc.py", "# 1", "# 2"), None);
        assert_eq!(
            changed("src/main.rs", "let count1 = 1;", "let count2 = 1;"),
            None
        );
        assert_eq!(
            changed(
                "src/main.rs",
                "let n = 1; let m = 2;",
                "let n = 3; let m = 4;"
            ),
            None
        );
        assert_eq!(changed("notes.txt", "n = 1", "n = 2"), None);
        assert_eq!(changed("notes.txt", "n = -1", "n = -2"), None);
    }

    #[test]
    fn replace_writes_the_one_sequence() {
        assert_eq!(
            replace_integer("src/main.rs", "let n = 1; let m = 1;", "1", "2", 2).unwrap(),
            "let n = 1; let m = 2;"
        );
        assert_eq!(
            replace_integer("src/main.rs", "let n = -1;", "-1", "-2", 1).unwrap(),
            "let n = -2;"
        );
        assert!(replace_integer("src/main.rs", "let n = a-1;", "-1", "-2", 1).is_err());
        assert!(replace_integer("src/main.rs", "let n = 1;", "1", "2", 2).is_err());
        assert!(replace_integer("src/main.rs", "let n = 1;", "9", "2", 1).is_err());
    }

    fn flipped(path: &str, old_line: &str, new_line: &str) -> Option<(String, String, usize)> {
        boolean_constant(path, old_line, new_line)
            .map(|constant| (constant.old_value, constant.new_value, constant.occurrence))
    }

    #[test]
    fn one_boolean_on_the_line_is_a_constant() {
        assert_eq!(
            flipped("src/main.rs", "let n = true;", "let n = false;"),
            Some(("true".to_string(), "false".to_string(), 1))
        );
        assert_eq!(
            flipped("src/main.rs", "let n = false;", "let n = true;"),
            Some(("false".to_string(), "true".to_string(), 1))
        );
        assert_eq!(
            flipped(
                "src/main.rs",
                "let a = true; let b = true;",
                "let a = true; let b = false;"
            ),
            Some(("true".to_string(), "false".to_string(), 2))
        );
        assert_eq!(
            flipped("src/main.js", "const n = true;", "const n = false;"),
            Some(("true".to_string(), "false".to_string(), 1))
        );
        assert_eq!(
            flipped("calc.py", "n = True", "n = False"),
            Some(("True".to_string(), "False".to_string(), 1))
        );
        assert_eq!(
            flipped("calc.py", "n = False", "n = True"),
            Some(("False".to_string(), "True".to_string(), 1))
        );
        assert!(is_boolean("src/main.rs", "true"));
        assert!(is_boolean("calc.py", "True"));
        assert!(!is_boolean("src/main.rs", "True"));
        assert!(!is_boolean("src/main.c", "true"));
        assert!(!is_boolean("notes.txt", "true"));
    }

    #[test]
    fn a_boolean_inside_a_name_string_or_comment_stays_put() {
        assert_eq!(
            flipped("src/main.rs", "let n = true_flag;", "let n = false_flag;"),
            None
        );
        assert_eq!(
            flipped("src/main.rs", "let s = \"true\";", "let s = \"false\";"),
            None
        );
        assert_eq!(
            flipped(
                "src/main.rs",
                "let s = \"true\"; let n = true;",
                "let s = \"true\"; let n = false;"
            ),
            Some(("true".to_string(), "false".to_string(), 2))
        );
        assert_eq!(
            flipped("src/main.js", "const s = 'true';", "const s = 'false';"),
            None
        );
        assert_eq!(
            flipped("calc.py", "s = '''True'''", "s = '''False'''"),
            None
        );
        assert_eq!(flipped("src/main.rs", "// true", "// false"), None);
        assert_eq!(flipped("calc.py", "# True", "# False"), None);
        assert_eq!(
            flipped(
                "src/main.rs",
                "let a = true; let b = false;",
                "let a = false; let b = true;"
            ),
            None
        );
        assert_eq!(
            flipped("src/main.c", "int n = true;", "int n = false;"),
            None
        );
        assert_eq!(flipped("calc.py", "n = true", "n = false"), None);
        assert_eq!(flipped("notes.txt", "n = true", "n = false"), None);
    }

    #[test]
    fn replace_writes_the_one_boolean() {
        assert_eq!(
            replace_boolean(
                "src/main.rs",
                "let a = true; let b = true;",
                "true",
                "false",
                2
            )
            .unwrap(),
            "let a = true; let b = false;"
        );
        assert_eq!(
            replace_boolean("src/main.rs", "let n = a-true;", "true", "false", 1).unwrap(),
            "let n = a-false;"
        );
        assert!(replace_boolean("src/main.rs", "let n = true;", "true", "false", 2).is_err());
        assert!(replace_boolean("src/main.rs", "let n = true;", "false", "true", 1).is_err());
        assert!(replace_boolean("src/main.c", "int n = true;", "true", "false", 1).is_err());
    }
}
