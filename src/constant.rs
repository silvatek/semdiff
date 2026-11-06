//! An integer whose text changed on one line.
//!
//! The edit is a constant change when the two lines differ only inside one
//! integer. An integer is a maximal ASCII digit sequence. A leading `-` is
//! part of it when that minus starts the number: nothing before it produces
//! a value, so the start of the line, an operator, or a word such as
//! `return` all qualify. A minus after a value is subtraction and stays on
//! the line, as in `count-1` or `items[i]-1`. A `--` glued to the digits is
//! an operator. The line is not a line comment, the digits are not inside a
//! string, and they do not continue an identifier. A file with no dialect is
//! not read for constants.

use crate::classify::{introduces_line_comment, span_in_string};
use crate::dialect::Dialect;

/// The old integer, the new integer, and which digit sequence on the old line changed.
///
/// `occurrence` counts maximal digit sequences on the old line, starting at 1.
/// A sequence inside a string or an identifier still counts, so the index
/// stays put when those runs are not the integer that changed.
pub(crate) struct IntegerConstant {
    pub(crate) old_value: String,
    pub(crate) new_value: String,
    pub(crate) occurrence: usize,
}

/// One integer on a line.
///
/// `start` includes a unary minus when that minus belongs to the number.
/// `digits` is the first digit, which is what `occurrence` counts.
struct IntegerToken {
    start: usize,
    digits: usize,
    end: usize,
}

/// The integer constant changed between these two lines, when that is the whole edit.
///
/// `None` leaves the edit as a line modification. The integer has to be the
/// only difference, so the text between integers matches and every other
/// integer matches. Shared digits at either end of a sequence belong to it,
/// so `12` becoming `13` is one integer.
pub(crate) fn integer_constant(
    path: &str,
    old_line: &str,
    new_line: &str,
) -> Option<IntegerConstant> {
    let dialect = crate::dialect::for_path(path)?;
    if introduces_line_comment(dialect, old_line) || introduces_line_comment(dialect, new_line) {
        return None;
    }
    let old_tokens = integer_tokens(dialect, old_line);
    let new_tokens = integer_tokens(dialect, new_line);
    if old_tokens.len() != new_tokens.len() {
        return None;
    }
    let mut changed = None;
    let mut old_from = 0;
    let mut new_from = 0;
    for (old_token, new_token) in old_tokens.iter().zip(new_tokens.iter()) {
        if old_line[old_from..old_token.start] != new_line[new_from..new_token.start] {
            return None;
        }
        let old_text = &old_line[old_token.start..old_token.end];
        let new_text = &new_line[new_token.start..new_token.end];
        if old_text != new_text {
            if changed.is_some()
                || span_in_string(dialect, old_line, old_token.digits, old_token.end)
                || span_in_string(dialect, new_line, new_token.digits, new_token.end)
            {
                return None;
            }
            let occurrence = digit_runs(old_line)
                .iter()
                .position(|(start, _)| *start == old_token.digits)?
                + 1;
            changed = Some(IntegerConstant {
                old_value: old_text.to_string(),
                new_value: new_text.to_string(),
                occurrence,
            });
        }
        old_from = old_token.end;
        new_from = new_token.end;
    }
    if old_line[old_from..] != new_line[new_from..] {
        return None;
    }
    changed
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

/// Integers on `line`, in order.
///
/// A digit sequence that continues an identifier is not an integer. It stays
/// in the text between integers, and it still counts toward `occurrence`.
fn integer_tokens(dialect: &Dialect, line: &str) -> Vec<IntegerToken> {
    digit_runs(line)
        .into_iter()
        .filter(|(digits, _)| !continues_identifier(dialect, line, *digits))
        .map(|(digits, end)| IntegerToken {
            start: signed_start(dialect, line, digits),
            digits,
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
    use super::{integer_constant, is_integer, replace_integer};

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
}
