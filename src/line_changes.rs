use crate::change::Change;

/// Line additions, removals, and modifications between two copies of one text file.
///
/// The copies are split into lines, and one longest common subsequence picks the
/// lines that stayed. A walk along those matches turns each gap into a hunk.
/// Lines in a hunk pair from the top into modifications. A modification whose
/// only change is one identifier, occurring once on the old line, is an
/// identifier rename. Leftover old lines were removed, and leftover new lines
/// were added.
///
/// Each record carries `old` and `new`. `old` is a line in the old file. For an
/// added line it is the old line the text is inserted before, or one past the
/// last old line when the text is appended. `new` is a line in the new file.
/// For a removed line it is the new line that follows the removal, or one past
/// the last new line when the removal is at the end. An identifier rename
/// records its old line in the scope, as `path:line`. Renames of one name in
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
/// removed. A line left over on the new side was added.
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
            false,
        )));
    }
    for line in &hunk.new_lines[paired..] {
        changes.push(Box::new(crate::change::LineAdded::new(
            path.to_string(),
            hunk.next_old_line,
            line.index + 1,
            line.text.clone(),
            false,
        )));
    }
    changes
}

/// A paired line, recorded as an identifier rename when that is the whole edit.
///
/// The rename's scope is `path:line`, with the old line.
fn modified_line(
    path: &str,
    old: usize,
    new: usize,
    old_text: &str,
    new_text: &str,
) -> Box<dyn Change> {
    if let Some(rename) = crate::identifier::identifier_rename(path, old_text, new_text) {
        return Box::new(crate::change::IdentifierRenamed::new(
            crate::change::SpanScope::new(path, old),
            new,
            rename.oldname,
            rename.newname,
        ));
    }
    Box::new(crate::change::LineModified::new(
        path.to_string(),
        old,
        new,
        new_text.to_string(),
        false,
    ))
}

/// `old`, then `new`, so the records walk the file from the top.
fn line_position(change: &dyn Change) -> (usize, usize) {
    change
        .line_edit()
        .map(|line| line.position())
        .unwrap_or((0, 0))
}
