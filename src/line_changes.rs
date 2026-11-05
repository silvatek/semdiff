/// Line additions, removals, and modifications between two copies of one text file.
///
/// The copies are split into lines, and one longest common subsequence picks the
/// lines that stayed. A walk along those matches turns each gap into a hunk.
/// Lines in a hunk pair from the top into modifications. Leftover old lines were
/// removed, and leftover new lines were added.

/// One line edit in a text file.
///
/// `line` is 1-based. An added line is numbered where it sits in the new file.
/// A removed or modified line is numbered where it sat in the old file.
pub(crate) fn line_changes(path: &str, before: &str, after: &str) -> Vec<crate::semdif::Change> {
    let old_lines = split_text(before);
    let new_lines = split_text(after);
    let matches = longest_common_subsequence(&old_lines.lines, &new_lines.lines);
    let mut changes = Vec::new();
    for hunk in hunks(&old_lines.lines, &new_lines.lines, &matches) {
        changes.extend(changes_in_hunk(path, &hunk));
    }
    replay_order(changes)
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
struct Hunk {
    old_lines: Vec<IndexedLine>,
    new_lines: Vec<IndexedLine>,
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
fn changes_in_hunk(path: &str, hunk: &Hunk) -> Vec<crate::semdif::Change> {
    let paired = hunk.old_lines.len().min(hunk.new_lines.len());
    let mut changes = Vec::new();
    for offset in 0..paired {
        changes.push(crate::semdif::Change::LineModified {
            path: path.to_string(),
            line: hunk.old_lines[offset].index + 1,
            text: hunk.new_lines[offset].text.clone(),
        });
    }
    for line in &hunk.old_lines[paired..] {
        changes.push(crate::semdif::Change::LineRemoved {
            path: path.to_string(),
            line: line.index + 1,
            text: line.text.clone(),
        });
    }
    for line in &hunk.new_lines[paired..] {
        changes.push(crate::semdif::Change::LineAdded {
            path: path.to_string(),
            line: line.index + 1,
            text: line.text.clone(),
        });
    }
    changes
}

/// The order in which line edits can be replayed on the old file.
///
/// Modifications keep every old line number valid. Removals then run from the
/// bottom, so a line number recorded against the old file still points at the
/// same line. Additions run from the top once those old lines are in place,
/// and their numbers are positions in the new file.
fn replay_order(changes: Vec<crate::semdif::Change>) -> Vec<crate::semdif::Change> {
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for change in changes {
        match change {
            crate::semdif::Change::LineModified { .. } => modified.push(change),
            crate::semdif::Change::LineRemoved { .. } => removed.push(change),
            crate::semdif::Change::LineAdded { .. } => added.push(change),
            other => modified.push(other),
        }
    }
    modified.sort_by_key(line_number);
    removed.sort_by_key(|change| std::cmp::Reverse(line_number(change)));
    added.sort_by_key(line_number);
    modified.extend(removed);
    modified.extend(added);
    modified
}

fn line_number(change: &crate::semdif::Change) -> usize {
    match change {
        crate::semdif::Change::LineAdded { line, .. }
        | crate::semdif::Change::LineRemoved { line, .. }
        | crate::semdif::Change::LineModified { line, .. } => *line,
        _ => 0,
    }
}
