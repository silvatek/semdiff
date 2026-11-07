//! Recognizing an import on one line, and replacing its name.
//!
//! A dialect lists the keywords that introduce an import. A line starts with
//! one of those keywords when the keyword is the first word, after whitespace
//! and before a line comment. The import name is the text after the keyword,
//! without a trailing semicolon. Replacing that name is how an import move is
//! replayed.

use crate::dialect::{dialect_for_path, Dialect};
use crate::diff::span_changes::line_comment_marker;

/// The old and new import names, when replacing the old name yields `new_line`.
///
/// Both lines have to start with an import keyword. The names have to differ.
/// Anything else on the line, including the keyword, the spacing, a trailing
/// semicolon, and a trailing line comment, has to stay. `None` leaves the
/// edit as a line modification.
pub(crate) fn import_move(path: &str, old_line: &str, new_line: &str) -> Option<(String, String)> {
    let dialect = dialect_for_path(path)?;
    let old = located_import(dialect, old_line)?;
    let new = located_import(dialect, new_line)?;
    if old.name == new.name {
        return None;
    }
    let replaced = splice(old_line, old.start, old.end, new.name);
    if replaced != new_line {
        return None;
    }
    Some((old.name.to_string(), new.name.to_string()))
}

/// How many times `name` is the import on this line.
///
/// A line has one import or none, so the count is 1 or 0. `None` means the
/// file has no dialect, so the line is not read for imports.
pub(crate) fn import_occurrence(path: &str, line: &str, name: &str) -> Option<usize> {
    let dialect = dialect_for_path(path)?;
    match located_import(dialect, line) {
        Some(import) if import.name == name => Some(1),
        _ => Some(0),
    }
}

/// Replaces the import name on this line when it is `oldname`.
///
/// A line that is not that import is an error, so apply does not write a
/// different edit. A file with no dialect is an error for the same reason.
pub(crate) fn replace_import(
    path: &str,
    line: &str,
    oldname: &str,
    newname: &str,
) -> Result<String, String> {
    let dialect = dialect_for_path(path)
        .ok_or_else(|| format!("{path} has no dialect for an import move"))?;
    let Some(import) = located_import(dialect, line) else {
        return Err(format!("{oldname} is not the import on this line"));
    };
    if import.name != oldname {
        return Err(format!("{oldname} is not the import on this line"));
    }
    Ok(splice(line, import.start, import.end, newname))
}

/// Replaces the import name on this line when it is `oldname`.
///
/// A line that is not that import stays as it is. A file with no dialect is
/// an error, so apply does not write a different edit.
pub(crate) fn replace_import_every(
    path: &str,
    line: &str,
    oldname: &str,
    newname: &str,
) -> Result<String, String> {
    let dialect = dialect_for_path(path)
        .ok_or_else(|| format!("{path} has no dialect for an import move"))?;
    let Some(import) = located_import(dialect, line) else {
        return Ok(line.to_string());
    };
    if import.name != oldname {
        return Ok(line.to_string());
    }
    Ok(splice(line, import.start, import.end, newname))
}

/// The import name on one line, and where it sits.
struct LocatedImport<'a> {
    name: &'a str,
    start: usize,
    end: usize,
}

/// The import on `line`, when the line starts with one of the dialect's keywords.
///
/// Leading whitespace is skipped. The keyword has to be a whole word. The name
/// is the text after the keyword, up to a trailing line comment, with the
/// surrounding whitespace and one trailing semicolon removed. An empty name
/// is not an import.
fn located_import<'a>(dialect: &Dialect, line: &'a str) -> Option<LocatedImport<'a>> {
    let content_end = line_comment_marker(dialect, line).unwrap_or(line.len());
    let head = &line[..content_end];
    let skipped = head.len() - head.trim_start().len();
    let body = head.trim_start();
    let keyword = matching_keyword(dialect, body)?;
    let after_keyword = skipped + keyword.len();
    let rest = &line[after_keyword..content_end];
    let name_pad = rest.len() - rest.trim_start().len();
    let start = after_keyword + name_pad;
    let tail = &line[start..content_end];
    let trimmed_tail = tail.trim_end();
    let name_len = import_name_len(trimmed_tail);
    if name_len == 0 {
        return None;
    }
    Some(LocatedImport {
        name: &line[start..start + name_len],
        start,
        end: start + name_len,
    })
}

/// The length of the import name at the start of `tail`.
///
/// `tail` has already had its trailing whitespace removed. One trailing
/// semicolon is statement punctuation, not part of the name, and whitespace
/// before that semicolon is not either.
fn import_name_len(tail: &str) -> usize {
    let Some(without_semicolon) = tail.strip_suffix(';') else {
        return tail.len();
    };
    without_semicolon.trim_end().len()
}

/// The longest import keyword that `text` starts with, as a whole word.
fn matching_keyword<'a>(dialect: &'a Dialect, text: &str) -> Option<&'a str> {
    dialect
        .import_keywords
        .iter()
        .copied()
        .filter(|keyword| keyword_starts(dialect, text, keyword))
        .max_by_key(|keyword| keyword.len())
}

/// Whether `text` starts with `keyword` and the next character does not continue a name.
fn keyword_starts(dialect: &Dialect, text: &str, keyword: &str) -> bool {
    if keyword.is_empty() {
        return false;
    }
    let Some(rest) = text.strip_prefix(keyword) else {
        return false;
    };
    match rest.chars().next() {
        None => true,
        Some(character) => !dialect.identifier_characters.continues(character),
    }
}

/// `line` with `line[start..end]` replaced by `name`.
fn splice(line: &str, start: usize, end: usize, name: &str) -> String {
    let mut replaced = String::new();
    replaced.push_str(&line[..start]);
    replaced.push_str(name);
    replaced.push_str(&line[end..]);
    replaced
}

#[cfg(test)]
mod tests {
    use super::import_move;

    fn names(path: &str, old_line: &str, new_line: &str) -> Option<(String, String)> {
        import_move(path, old_line, new_line)
    }

    #[test]
    fn a_line_that_starts_with_an_import_keyword_names_the_import() {
        assert_eq!(
            names(
                "src/main.rs",
                "use old::path::Item;",
                "use new::place::Item;"
            ),
            Some((
                "old::path::Item".to_string(),
                "new::place::Item".to_string()
            ))
        );
        assert_eq!(
            names(
                "src/main.rs",
                "    use old::path::Item;",
                "    use new::place::Item;"
            ),
            Some((
                "old::path::Item".to_string(),
                "new::place::Item".to_string()
            ))
        );
        assert_eq!(
            names("pkg/app.py", "import old.pkg.name", "import new.mod.name"),
            Some(("old.pkg.name".to_string(), "new.mod.name".to_string()))
        );
        assert_eq!(
            names(
                "pkg/app.py",
                "from old.pkg import name",
                "from new.mod import name"
            ),
            Some((
                "old.pkg import name".to_string(),
                "new.mod import name".to_string()
            ))
        );
        assert_eq!(
            names(
                "src/app.js",
                "import { read } from \"./old/path.js\";",
                "import { read } from \"./new/place.js\";"
            ),
            Some((
                "{ read } from \"./old/path.js\"".to_string(),
                "{ read } from \"./new/place.js\"".to_string()
            ))
        );
        assert_eq!(
            names(
                "src/main.c",
                "#include \"old/path.h\"",
                "#include \"new/place.h\""
            ),
            Some(("\"old/path.h\"".to_string(), "\"new/place.h\"".to_string()))
        );
        assert_eq!(
            names(
                "src/main.c",
                "#include \"http://old/a.h\"",
                "#include \"http://new/b.h\""
            ),
            Some((
                "\"http://old/a.h\"".to_string(),
                "\"http://new/b.h\"".to_string()
            ))
        );
    }

    #[test]
    fn a_trailing_comment_stays_outside_the_import_name() {
        assert_eq!(
            names(
                "src/main.rs",
                "use old::path::Item; // keep",
                "use new::place::Item; // keep"
            ),
            Some((
                "old::path::Item".to_string(),
                "new::place::Item".to_string()
            ))
        );
        assert!(names(
            "src/main.rs",
            "use old::path::Item; // keep",
            "use new::place::Item; // changed"
        )
        .is_none());
        assert!(names(
            "pkg/app.py",
            "import old.pkg.name  # keep",
            "import new.mod.name  # changed"
        )
        .is_none());
    }

    #[test]
    fn a_line_that_does_not_introduce_an_import_stays_out() {
        assert!(names("src/main.rs", "useful tool;", "useful tools;").is_none());
        assert!(names("pkg/app.py", "important = 1", "important = 2").is_none());
        assert!(names(
            "src/main.rs",
            "pub use old::path::Item;",
            "pub use new::place::Item;"
        )
        .is_none());
        assert!(names(
            "src/main.rs",
            "// use old::path::Item;",
            "// use new::place::Item;"
        )
        .is_none());
        assert!(names("src/main.rs", "use", "use new::place::Item;").is_none());
        assert!(names("src/main.rs", "use ;", "use new::place::Item;").is_none());
        assert!(names(
            "src/main.rs",
            "use old::path::Item;",
            "use old::path::Item;"
        )
        .is_none());
        assert!(names("pkg/app.py", "from old.pkg import name", "import old.pkg").is_none());
        assert!(names("notes.txt", "use old::path::Item;", "use new::place::Item;").is_none());
        assert!(names(
            "src/main.rs",
            "use old::path::Item;",
            "use  new::place::Item;"
        )
        .is_none());
    }
}
