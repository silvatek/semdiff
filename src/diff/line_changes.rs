use crate::change::Change;

/// Line records for two copies of one file, when both copies are text.
///
/// `None` means a copy is not text, so the caller records the file as a whole.
/// An empty list means the lines match.
pub(crate) fn text_line_changes(
    path: &str,
    before: &[u8],
    after: &[u8],
) -> Option<Vec<Box<dyn Change>>> {
    let (Ok(before_text), Ok(after_text)) =
        (std::str::from_utf8(before), std::str::from_utf8(after))
    else {
        return None;
    };
    Some(line_changes(path, before_text, after_text))
}

/// Line additions, removals, and modifications between two copies of one text file.
///
/// The copies are split into lines, and one longest common subsequence picks the
/// lines that stayed. A walk along those matches turns each gap into a hunk.
/// Lines in a hunk pair from the top into modifications. A modification whose
/// only change is one identifier, occurring once on the old line, is an
/// identifier rename. A changed stretch of that line is a span, widened to
/// the token that contains it. One such span is a constant change when it is
/// a boolean spelling or an integer outside a string and a comment, and an
/// identifier rename when it is one name. A leading `-` is part of the integer
/// when it starts the number. ANSI C lists no boolean spellings, so those
/// words stay names. Several changed spans stay a line modification.
/// An added or removed line is a full-line comment when that line opens with
/// the dialect's line comment. A modification is one when
/// both the old line and the new line do. A file with no dialect is never a
/// comment. An added or removed line is blank when it holds only ASCII spaces
/// and tabs. A modification is margin whitespace when the dialect's reader
/// treats the difference as formatting. Leftover old lines were removed, and
/// leftover new lines were added.
///
/// Each record carries `old` and `new`. `old` is a line in the old file. For an
/// added line it is the old line the text is inserted before, or one past the
/// last old line when the text is appended. `new` is a line in the new file.
/// For a removed line it is the new line that follows the removal, or one past
/// the last new line when the removal is at the end. An identifier rename
/// records its old line in the scope, as `path:line`. A constant change does
/// the same. Renames of one name in
/// that file become `path:start-end` when every occurrence of the old name
/// between them is one of those renames, and that run does not record `new`.
/// The records are ordered by
/// `old`, then `new`, which walks the file from the top. Apply rebuilds the
/// file from the old positions, so that order is for reading.
pub(crate) fn line_changes(path: &str, before: &str, after: &str) -> Vec<Box<dyn Change>> {
    let old_lines = split_text(before);
    let new_lines = split_text(after);
    let matches = longest_common_subsequence(&old_lines.lines, &new_lines.lines);
    let mut changes: Vec<Box<dyn Change>> = Vec::new();
    for hunk in hunks(&old_lines.lines, &new_lines.lines, &matches) {
        changes.extend(changes_in_hunk(path, &hunk));
    }
    changes.sort_by_key(|change| line_position(change.as_ref()));
    crate::change::merge_identifier_spans(path, &old_lines.lines, changes)
}

/// The lines of a text file, without their separating newlines.
///
/// A trailing newline is remembered so the file can be written back unchanged.
/// It is not itself a line. A carriage return stays on the line it follows.
pub(crate) struct Text {
    pub(crate) lines: Vec<String>,
    pub(crate) trailing_newline: bool,
}

pub(crate) fn split_text(text: &str) -> Text {
    if text.is_empty() {
        return Text {
            lines: Vec::new(),
            trailing_newline: false,
        };
    }
    let trailing_newline = text.ends_with('\n');
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    if trailing_newline {
        lines.pop();
    }
    Text {
        lines,
        trailing_newline,
    }
}

pub(crate) fn join_text(text: &Text) -> String {
    let mut joined = text.lines.join("\n");
    if text.trailing_newline {
        joined.push('\n');
    }
    joined
}

/// One line that occurs in both copies, in the same order.
struct CommonLine {
    old_index: usize,
    new_index: usize,
}

/// The longest list of lines that occurs in both copies, in the same order.
///
/// A table records how long that list is for every prefix of the two files.
/// When the current lines are equal, the list is one longer than the list for
/// the prefixes that end just before them. Otherwise it is the longer of the
/// two prefixes that drop one of the current lines. Walking back from the
/// bottom-right cell recovers one such list.
fn longest_common_subsequence(old_lines: &[String], new_lines: &[String]) -> Vec<CommonLine> {
    let lengths = common_lengths(old_lines, new_lines);
    recover_common_lines(old_lines, new_lines, &lengths)
}

fn common_lengths(old_lines: &[String], new_lines: &[String]) -> Vec<Vec<usize>> {
    let mut lengths = vec![vec![0; new_lines.len() + 1]; old_lines.len() + 1];
    for old_index in 0..old_lines.len() {
        for new_index in 0..new_lines.len() {
            lengths[old_index + 1][new_index + 1] = if old_lines[old_index] == new_lines[new_index]
            {
                lengths[old_index][new_index] + 1
            } else {
                lengths[old_index][new_index + 1].max(lengths[old_index + 1][new_index])
            };
        }
    }
    lengths
}

fn recover_common_lines(
    old_lines: &[String],
    new_lines: &[String],
    lengths: &[Vec<usize>],
) -> Vec<CommonLine> {
    let mut matched = Vec::new();
    let mut old_index = old_lines.len();
    let mut new_index = new_lines.len();
    while old_index > 0 && new_index > 0 {
        if old_lines[old_index - 1] == new_lines[new_index - 1] {
            matched.push(CommonLine {
                old_index: old_index - 1,
                new_index: new_index - 1,
            });
            old_index -= 1;
            new_index -= 1;
        } else if lengths[old_index - 1][new_index] >= lengths[old_index][new_index - 1] {
            old_index -= 1;
        } else {
            new_index -= 1;
        }
    }
    matched.reverse();
    matched
}

/// Old and new lines that sit in one gap between common lines.
///
/// `next_old_line` and `next_new_line` are the 1-based lines just after the gap.
/// An addition is inserted before `next_old_line`. A removal is followed by
/// `next_new_line`. Either one is one past the end when the gap reaches the end.
struct Hunk {
    old_lines: Vec<IndexedLine>,
    new_lines: Vec<IndexedLine>,
    next_old_line: usize,
    next_new_line: usize,
}

struct IndexedLine {
    index: usize,
    text: String,
}

/// The gaps between matched lines, including anything before the first match and after the last.
fn hunks(old_lines: &[String], new_lines: &[String], matches: &[CommonLine]) -> Vec<Hunk> {
    let mut found = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;
    for common in matches {
        push_hunk(
            &mut found,
            old_lines,
            new_lines,
            old_index,
            common.old_index,
            new_index,
            common.new_index,
        );
        old_index = common.old_index + 1;
        new_index = common.new_index + 1;
    }
    push_hunk(
        &mut found,
        old_lines,
        new_lines,
        old_index,
        old_lines.len(),
        new_index,
        new_lines.len(),
    );
    found
}

fn push_hunk(
    hunks: &mut Vec<Hunk>,
    old_lines: &[String],
    new_lines: &[String],
    old_from: usize,
    old_to: usize,
    new_from: usize,
    new_to: usize,
) {
    if old_from == old_to && new_from == new_to {
        return;
    }
    hunks.push(Hunk {
        old_lines: indexed(old_lines, old_from, old_to),
        new_lines: indexed(new_lines, new_from, new_to),
        next_old_line: old_to + 1,
        next_new_line: new_to + 1,
    });
}

fn indexed(lines: &[String], from: usize, to: usize) -> Vec<IndexedLine> {
    lines[from..to]
        .iter()
        .enumerate()
        .map(|(offset, text)| IndexedLine {
            index: from + offset,
            text: text.clone(),
        })
        .collect()
}

/// Modifications for the paired lines in a hunk, then removals, then additions.
///
/// Pairing runs from the top of the hunk. A line left over on the old side was
/// removed. A line left over on the new side was added. A blank line, empty
/// or holding only ASCII spaces and tabs, is whitespace when the file has a
/// dialect.
fn changes_in_hunk(path: &str, hunk: &Hunk) -> Vec<Box<dyn Change>> {
    let paired = hunk.old_lines.len().min(hunk.new_lines.len());
    let mut changes: Vec<Box<dyn Change>> = Vec::new();
    for offset in 0..paired {
        changes.push(modified_line(
            path,
            hunk.old_lines[offset].index + 1,
            hunk.new_lines[offset].index + 1,
            &hunk.old_lines[offset].text,
            &hunk.new_lines[offset].text,
        ));
    }
    for line in &hunk.old_lines[paired..] {
        changes.push(Box::new(crate::change::LineRemoved::new(
            path.to_string(),
            line.index + 1,
            hunk.next_new_line,
            line.text.clone(),
            full_line_comment(path, &line.text),
            blank_line(path, &line.text),
        )));
    }
    for line in &hunk.new_lines[paired..] {
        changes.push(Box::new(crate::change::LineAdded::new(
            path.to_string(),
            hunk.next_old_line,
            line.index + 1,
            line.text.clone(),
            full_line_comment(path, &line.text),
            blank_line(path, &line.text),
        )));
    }
    changes
}

/// A paired line, recorded as a boolean constant, an identifier rename, or an
/// integer constant when that is the whole edit.
///
/// The scope of those records is `path:line`, with the old line. Anything else
/// is a line modification. It is a full-line comment when both lines open with the
/// dialect's line comment. It is whitespace when the edit is a blank line or
/// only an insignificant margin.
fn modified_line(
    path: &str,
    old: usize,
    new: usize,
    old_text: &str,
    new_text: &str,
) -> Box<dyn Change> {
    let found = super::span::find_spans(path, old_text, new_text);
    if let [span] = found.as_slice() {
        if let Some(change) = super::span_changes::record(path, old, new, span) {
            return change;
        }
    }
    Box::new(crate::change::LineModified::new(
        path.to_string(),
        old,
        new,
        new_text.to_string(),
        full_line_comment(path, old_text) && full_line_comment(path, new_text),
        margin_whitespace(path, old_text, new_text),
    ))
}

/// `old`, then `new`, so the records walk the file from the top.
fn line_position(change: &dyn Change) -> (usize, usize) {
    change
        .line_edit()
        .map(|line| line.position())
        .unwrap_or((0, 0))
}

/// Whether `line` introduces this dialect's line comment before any other text.
///
/// Leading whitespace is skipped. The line-comment marker then has to be the
/// next characters. A block comment is not recognized here.
fn introduces_line_comment(dialect: &crate::dialect::Dialect, line: &str) -> bool {
    line.trim_start().starts_with(dialect.line_comment)
}

/// Whether `text` is a full-line comment for the file at `path`.
///
/// Every line has to introduce the dialect's line comment. A file with no
/// dialect is not a comment. An empty line is not one either.
fn full_line_comment(path: &str, text: &str) -> bool {
    let Some(dialect) = crate::dialect::for_path(path) else {
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
/// A blank line is empty or holds only ASCII spaces and tabs. A known dialect
/// ignores that line, so editing one has no functional effect. A file with
/// no dialect is not blank for this purpose. A comment, a carriage return, or
/// any other character keeps the line unflagged.
fn blank_line(path: &str, text: &str) -> bool {
    if crate::dialect::for_path(path).is_none() {
        return false;
    }
    text.split('\n').all(only_ascii_blank)
}

/// Whether `line` contains nothing but ASCII spaces and tabs.
fn only_ascii_blank(line: &str) -> bool {
    line.chars()
        .all(|character| character == ' ' || character == '\t')
}

/// Whether changing `old_line` into `new_line` touches only an insignificant margin.
///
/// The dialect's reader decides. In the C family, braces mark the block, so
/// indentation is formatting. In Python, a leading indent change is a block
/// boundary. Trailing space is insignificant in either reader. A file with no
/// dialect is not whitespace. A carriage return stays part of the line.
fn margin_whitespace(path: &str, old_line: &str, new_line: &str) -> bool {
    let Some(dialect) = crate::dialect::for_path(path) else {
        return false;
    };
    match dialect.family {
        crate::dialect::Family::CFamily => {
            crate::family::c_family::margin_whitespace(old_line, new_line)
        }
        crate::dialect::Family::Python => {
            crate::family::python::margin_whitespace(dialect, old_line, new_line)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blank_line, full_line_comment, introduces_line_comment, margin_whitespace};
    use crate::dialect::{ANSI_C, JAVASCRIPT, PYTHON, RUST};

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
        assert!(introduces_line_comment(&JAVASCRIPT, "// note"));
        assert!(introduces_line_comment(&JAVASCRIPT, "    // note"));
        assert!(!introduces_line_comment(
            &JAVASCRIPT,
            "const x = 1; // note"
        ));
        assert!(!introduces_line_comment(&JAVASCRIPT, "/* block */"));
        assert!(introduces_line_comment(&ANSI_C, "// note"));
        assert!(introduces_line_comment(&ANSI_C, "    // note"));
        assert!(!introduces_line_comment(&ANSI_C, "int n; // note"));
        assert!(!introduces_line_comment(&ANSI_C, "/* note */"));
    }

    #[test]
    fn a_full_line_comment_needs_a_known_dialect() {
        assert!(full_line_comment("src/main.rs", "// note"));
        assert!(full_line_comment("src/main.rs", "// one\n// two"));
        assert!(!full_line_comment("src/main.rs", "// one\nlet x = 1;"));
        assert!(!full_line_comment("src/main.rs", "/* note */"));
        assert!(full_line_comment("calc.py", "# note"));
        assert!(full_line_comment("src/main.js", "// note"));
        assert!(full_line_comment("src/main.js", "// one\n// two"));
        assert!(!full_line_comment("src/main.js", "/* note */"));
        assert!(full_line_comment("src/main.c", "// note"));
        assert!(!full_line_comment("src/main.c", "/* note */"));
        assert!(!full_line_comment("src/main.c", "int n;"));
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
        assert!(blank_line("src/main.js", ""));
        assert!(blank_line("src/main.js", "  "));
        assert!(blank_line("src/main.c", ""));
        assert!(blank_line("src/main.c", "  "));
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
        assert!(margin_whitespace(
            "src/main.js",
            "let value = 1;",
            "    let value = 1;"
        ));
        assert!(margin_whitespace(
            "src/main.js",
            "const trailing = 1;",
            "const trailing = 1; "
        ));
        assert!(margin_whitespace(
            "src/main.js",
            "// margin",
            "    // margin"
        ));
        assert!(margin_whitespace(
            "src/main.c",
            "int value = 1;",
            "    int value = 1;"
        ));
        assert!(margin_whitespace(
            "src/main.c",
            "int trailing = 1;",
            "int trailing = 1; "
        ));
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
}
