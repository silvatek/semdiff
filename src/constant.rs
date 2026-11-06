//! An integer whose digits changed on one line.
//!
//! The edit is a constant change when the two lines differ only inside one
//! sequence of digits. The line is not a line comment, the digits are not
//! inside a string, and they do not continue an identifier. A sign or a suffix
//! that stays put is left on the line. A file with no dialect is not read for
//! constants.

use crate::classify::{introduces_line_comment, span_in_string};
use crate::dialect::Dialect;

/// The old digits, the new digits, and which sequence on the old line changed.
///
/// `occurrence` counts maximal digit sequences on the old line, starting at 1.
/// A sequence inside a string still counts, so the index stays put.
pub(crate) struct IntegerConstant {
    pub(crate) old_value: String,
    pub(crate) new_value: String,
    pub(crate) occurrence: usize,
}

/// The integer constant changed between these two lines, when that is the whole edit.
///
/// `None` leaves the edit as a line modification. The digit sequence has to be
/// the only difference. Shared digits at either end of the sequence belong to
/// it, so `12` becoming `13` is one integer.
pub(crate) fn integer_constant(
    path: &str,
    old_line: &str,
    new_line: &str,
) -> Option<IntegerConstant> {
    let dialect = crate::dialect::for_path(path)?;
    if introduces_line_comment(dialect, old_line) || introduces_line_comment(dialect, new_line) {
        return None;
    }
    let (old_value, new_value, start) = changed_integer(old_line, new_line)?;
    if continues_identifier(dialect, old_line, start) {
        return None;
    }
    let old_end = start + old_value.len();
    let new_end = start + new_value.len();
    if span_in_string(dialect, old_line, start, old_end)
        || span_in_string(dialect, new_line, start, new_end)
    {
        return None;
    }
    let occurrence = digit_runs(old_line)
        .iter()
        .position(|(run_start, _)| *run_start == start)?
        + 1;
    Some(IntegerConstant {
        old_value,
        new_value,
        occurrence,
    })
}

/// Replaces one digit sequence on `line`.
///
/// `occurrence` is the 1-based index among maximal digit sequences. The text
/// of that sequence has to be `old_value`. A missing sequence is an error, so
/// apply does not write a different edit.
pub(crate) fn replace_integer(
    line: &str,
    old_value: &str,
    new_value: &str,
    occurrence: usize,
) -> Result<String, String> {
    let Some((start, end)) = digit_runs(line).get(occurrence.wrapping_sub(1)).copied() else {
        return Err(format!("integer {old_value} does not occur"));
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

/// Whether `text` is a non-empty sequence of ASCII digits.
pub(crate) fn is_digit_sequence(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| character.is_ascii_digit())
}

/// The old digits, the new digits, and the byte offset where both sequences start.
///
/// The lines have to match outside one digit sequence. Digits that the two
/// lines share at the ends of that sequence are part of it.
fn changed_integer(old_line: &str, new_line: &str) -> Option<(String, String, usize)> {
    let prefix = common_prefix_len(old_line, new_line);
    let old_rest = &old_line[prefix..];
    let new_rest = &new_line[prefix..];
    let suffix = common_suffix_len(old_rest, new_rest);
    let old_mid = &old_rest[..old_rest.len() - suffix];
    let new_mid = &new_rest[..new_rest.len() - suffix];
    if old_mid.is_empty() && new_mid.is_empty() {
        return None;
    }
    if !old_mid.chars().all(|character| character.is_ascii_digit())
        || !new_mid.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let start = expand_left(old_line, prefix);
    let old_end = expand_right(old_line, old_line.len() - suffix);
    let new_end = expand_right(new_line, new_line.len() - suffix);
    let old_value = &old_line[start..old_end];
    let new_value = &new_line[start..new_end];
    if !is_digit_sequence(old_value) || !is_digit_sequence(new_value) || old_value == new_value {
        return None;
    }
    Some((old_value.to_string(), new_value.to_string(), start))
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

fn common_prefix_len(old_line: &str, new_line: &str) -> usize {
    old_line
        .chars()
        .zip(new_line.chars())
        .take_while(|(old, new)| old == new)
        .map(|(character, _)| character.len_utf8())
        .sum()
}

fn common_suffix_len(old_rest: &str, new_rest: &str) -> usize {
    old_rest
        .chars()
        .rev()
        .zip(new_rest.chars().rev())
        .take_while(|(old, new)| old == new)
        .map(|(character, _)| character.len_utf8())
        .sum()
}

fn expand_left(line: &str, mut index: usize) -> usize {
    while index > 0 {
        let Some(character) = line[..index].chars().next_back() else {
            break;
        };
        if !character.is_ascii_digit() {
            break;
        }
        index -= character.len_utf8();
    }
    index
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
    use super::{integer_constant, replace_integer};

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
    }

    #[test]
    fn a_string_a_comment_or_a_name_stays_a_modification() {
        assert_eq!(
            changed("src/main.rs", "let s = \"1\";", "let s = \"2\";"),
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
    }

    #[test]
    fn replace_writes_the_one_sequence() {
        assert_eq!(
            replace_integer("let n = 1; let m = 1;", "1", "2", 2).unwrap(),
            "let n = 1; let m = 2;"
        );
        assert!(replace_integer("let n = 1;", "1", "2", 2).is_err());
        assert!(replace_integer("let n = 1;", "9", "2", 1).is_err());
    }
}
