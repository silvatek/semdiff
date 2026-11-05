//! Line additions, removals, and modifications.
//!
//! A line change's scope is the file. A span change names one old line instead,
//! and lives in its own module. The same line record can say that the edit is
//! a full-line comment, and that it is insignificant whitespace. Those are
//! `isComment: true` and `isWhitespace: true` in the semdif. False is the
//! default and is left out. A record may carry both. An addition the dialect
//! reads as a new function, method, or structure is `named-block-added` and
//! carries that block's name.

use std::path::Path;

use super::{applied_with_its_file, Change, Content, LineEdit, LinePlan, LineStart};

use crate::filesystem::FileSystem;

/// The line numbers and text shared by a line addition, removal, and modification.
///
/// The scope stays on each of those changes. These three differ in how long
/// the edit is on the old side and the new side, and in whether a following
/// edit of the same kind can be appended.
struct LineBody {
    old: usize,
    new: usize,
    text: String,
    /// Whether this edit is a full-line comment.
    ///
    /// Written as `isComment: true`. False is omitted from the semdif.
    is_comment: bool,
    /// Whether this edit is insignificant whitespace.
    ///
    /// Written as `isWhitespace: true`. False is omitted from the semdif.
    /// Apply replays the line text and does not read this flag.
    is_whitespace: bool,
}

fn line_count(text: &str) -> usize {
    text.split('\n').count()
}

/// Joins `next` onto `current` when it continues the same kind, scope, and flags.
///
/// `old_len` and `new_len` are how far `current` already reaches. The next
/// change continues it when each side's first line plus that length equals
/// the next change's first line. The joined text is stored on `current`.
/// A comment does not join a line that is not a comment, and a whitespace
/// edit does not join a line that is not whitespace, so those stay two records.
fn extend_following(
    current: &mut dyn Change,
    old_len: usize,
    new_len: usize,
    next: &dyn Change,
) -> bool {
    if next.kind() != current.kind() || next.scope() != current.scope() {
        return false;
    }
    let Some(line) = next.line_edit() else {
        return false;
    };
    let Some(following) = line.start() else {
        return false;
    };
    let (old, new, current_is_comment, current_is_whitespace) = match current.line_edit() {
        Some(current_line) => {
            let (old, new) = current_line.position();
            (
                old,
                new,
                current_line.is_comment(),
                current_line.is_whitespace(),
            )
        }
        None => return false,
    };
    if line.is_comment() != current_is_comment || line.is_whitespace() != current_is_whitespace {
        return false;
    }
    let Some(next_old) = old.checked_add(old_len) else {
        return false;
    };
    let Some(next_new) = new.checked_add(new_len) else {
        return false;
    };
    if next_old != following.old || next_new != following.new {
        return false;
    }
    let Some(text) = line.text() else {
        return false;
    };
    let Some(current_line) = current.line_edit_mut() else {
        return false;
    };
    current_line.append_text(text)
}

/// Stores another line on this edit, after the lines already there.
fn append_line(body: &mut LineBody, text: &str) {
    body.text.push('\n');
    body.text.push_str(text);
}

fn write_line(document: &mut String, body: &LineBody, lines: isize) {
    crate::semdif::write_plain(document, "old", &body.old.to_string());
    crate::semdif::write_plain(document, "lines", &format_lines(lines));
    crate::semdif::write_plain(document, "new", &body.new.to_string());
    crate::semdif::write_detail(document, "text", &body.text);
    if body.is_comment {
        crate::semdif::write_plain(document, "isComment", "true");
    }
    if body.is_whitespace {
        crate::semdif::write_plain(document, "isWhitespace", "true");
    }
}

fn format_lines(lines: isize) -> String {
    if lines > 0 {
        format!("+{lines}")
    } else if lines == 0 {
        "+0".to_string()
    } else {
        lines.to_string()
    }
}

fn line_start(body: &LineBody) -> LineStart {
    LineStart {
        old: body.old,
        new: body.new,
    }
}

/// Collapses adjacent line edits before they are written to the semdif file.
///
/// A run of line edits in one scope becomes one change when every line is a
/// comment, or none of them is. Removals follow one another on the old side
/// and share the new line that follows them, so `lines` is `-1` or `-2`.
/// Additions share an insertion point and follow one another on the new side,
/// so `lines` is `+1` or `+2`. Modifications follow one another on both sides.
/// They do not change the line count, so `lines` stays `+0`. The merged change
/// keeps the first line number. Its text keeps every line, joined by a newline.
/// A comment stays apart from a neighboring line that is not a comment. A
/// whitespace edit stays apart from a line that is not whitespace. A span
/// change stays its own change.
pub(crate) fn merge_adjacent(changes: Vec<Box<dyn super::Change>>) -> Vec<Box<dyn super::Change>> {
    let mut merged: Vec<Box<dyn super::Change>> = Vec::new();
    for change in changes {
        let extended = match merged
            .last_mut()
            .and_then(|previous| previous.line_edit_mut())
        {
            Some(line) => line.extend(change.as_ref()),
            None => false,
        };
        if !extended {
            merged.push(change);
        }
    }
    merged
}

/// Records a line addition as a named block when the dialect says it opens one.
///
/// Merging has already joined adjacent added lines into one record. This pass
/// reads that record and, when its first line opens a function, method, or
/// structure, replaces it with [`NamedBlockAdded`]. The line text is unchanged,
/// so apply inserts the same lines. A comment, whitespace, or an addition the
/// dialect does not treat as a named block stays a line addition.
pub(crate) fn promote_named_blocks(
    changes: Vec<Box<dyn super::Change>>,
) -> Vec<Box<dyn super::Change>> {
    changes.into_iter().map(promote_named_block).collect()
}

fn promote_named_block(change: Box<dyn super::Change>) -> Box<dyn super::Change> {
    match named_block_from_addition(change.as_ref()) {
        Some(promoted) => Box::new(promoted),
        None => change,
    }
}

fn named_block_from_addition(change: &dyn super::Change) -> Option<NamedBlockAdded> {
    if change.kind() != "line-added" {
        return None;
    }
    let line = change.line_edit()?;
    if line.is_comment() || line.is_whitespace() {
        return None;
    }
    let text = line.text()?;
    let name = crate::dialect::named_block(change.scope(), text)?;
    let (old, new) = line.position();
    Some(NamedBlockAdded::new(
        change.scope().to_string(),
        old,
        new,
        text.to_string(),
        name,
    ))
}

pub(crate) struct LineAdded {
    scope: String,
    body: LineBody,
}

impl LineAdded {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        is_comment: bool,
        is_whitespace: bool,
    ) -> Self {
        Self {
            scope,
            body: LineBody {
                old,
                new,
                text,
                is_comment,
                is_whitespace,
            },
        }
    }
}

impl Change for LineAdded {
    fn kind(&self) -> &'static str {
        "line-added"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn line_edit_mut(&mut self) -> Option<&mut dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        write_line(document, &self.body, line_count(&self.body.text) as isize);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for LineAdded {
    fn file(&self) -> &str {
        &self.scope
    }
}

impl LineEdit for LineAdded {
    fn position(&self) -> (usize, usize) {
        (self.body.old, self.body.new)
    }

    fn start(&self) -> Option<LineStart> {
        Some(line_start(&self.body))
    }

    fn text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn append_text(&mut self, text: &str) -> bool {
        append_line(&mut self.body, text);
        true
    }

    fn is_comment(&self) -> bool {
        self.body.is_comment
    }

    fn is_whitespace(&self) -> bool {
        self.body.is_whitespace
    }

    fn extend(&mut self, next: &dyn Change) -> bool {
        let new_len = line_count(&self.body.text);
        extend_following(self, 0, new_len, next)
    }

    fn rebuild(
        &self,
        file: &Path,
        _source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        plan.insert(file, self.body.old, self.body.new, &self.body.text)
    }
}

/// A line addition that opens a named block.
///
/// The block is a function, method, or structure, and `name` is the identifier
/// that follows the dialect's keyword. The record keeps the added lines, and
/// apply inserts them the way a line addition does. It does not join a
/// neighboring edit: the name belongs to this one block.
pub(crate) struct NamedBlockAdded {
    scope: String,
    body: LineBody,
    name: String,
}

impl NamedBlockAdded {
    pub(crate) fn new(scope: String, old: usize, new: usize, text: String, name: String) -> Self {
        Self {
            scope,
            body: LineBody {
                old,
                new,
                text,
                is_comment: false,
                is_whitespace: false,
            },
            name,
        }
    }
}

impl Change for NamedBlockAdded {
    fn kind(&self) -> &'static str {
        "named-block-added"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn line_edit_mut(&mut self) -> Option<&mut dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        write_line(document, &self.body, line_count(&self.body.text) as isize);
        crate::semdif::write_detail(document, "name", &self.name);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for NamedBlockAdded {
    fn file(&self) -> &str {
        &self.scope
    }
}

impl LineEdit for NamedBlockAdded {
    fn position(&self) -> (usize, usize) {
        (self.body.old, self.body.new)
    }

    fn start(&self) -> Option<LineStart> {
        Some(line_start(&self.body))
    }

    fn text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn is_comment(&self) -> bool {
        false
    }

    fn is_whitespace(&self) -> bool {
        false
    }

    fn rebuild(
        &self,
        file: &Path,
        _source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        plan.insert(file, self.body.old, self.body.new, &self.body.text)
    }
}

pub(crate) struct LineRemoved {
    scope: String,
    body: LineBody,
}

impl LineRemoved {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        is_comment: bool,
        is_whitespace: bool,
    ) -> Self {
        Self {
            scope,
            body: LineBody {
                old,
                new,
                text,
                is_comment,
                is_whitespace,
            },
        }
    }
}

impl Change for LineRemoved {
    fn kind(&self) -> &'static str {
        "line-removed"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn line_edit_mut(&mut self) -> Option<&mut dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        let count = line_count(&self.body.text) as isize;
        write_line(document, &self.body, -count);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for LineRemoved {
    fn file(&self) -> &str {
        &self.scope
    }
}

impl LineEdit for LineRemoved {
    fn position(&self) -> (usize, usize) {
        (self.body.old, self.body.new)
    }

    fn start(&self) -> Option<LineStart> {
        Some(line_start(&self.body))
    }

    fn text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn append_text(&mut self, text: &str) -> bool {
        append_line(&mut self.body, text);
        true
    }

    fn is_comment(&self) -> bool {
        self.body.is_comment
    }

    fn is_whitespace(&self) -> bool {
        self.body.is_whitespace
    }

    fn extend(&mut self, next: &dyn Change) -> bool {
        let old_len = line_count(&self.body.text);
        extend_following(self, old_len, 0, next)
    }

    fn rebuild(
        &self,
        file: &Path,
        _source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        let count = line_count(&self.body.text);
        let old_end = self
            .body
            .old
            .checked_add(count - 1)
            .ok_or_else(|| format!("line {} is outside {}", self.body.old, file.display()))?;
        plan.remove_range(file, self.body.old, old_end)
    }
}

pub(crate) struct LineModified {
    scope: String,
    body: LineBody,
}

impl LineModified {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        is_comment: bool,
        is_whitespace: bool,
    ) -> Self {
        Self {
            scope,
            body: LineBody {
                old,
                new,
                text,
                is_comment,
                is_whitespace,
            },
        }
    }
}

impl Change for LineModified {
    fn kind(&self) -> &'static str {
        "line-modified"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn line_edit_mut(&mut self) -> Option<&mut dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        write_line(document, &self.body, 0);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for LineModified {
    fn file(&self) -> &str {
        &self.scope
    }
}

impl LineEdit for LineModified {
    fn position(&self) -> (usize, usize) {
        (self.body.old, self.body.new)
    }

    fn start(&self) -> Option<LineStart> {
        Some(line_start(&self.body))
    }

    fn text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn append_text(&mut self, text: &str) -> bool {
        append_line(&mut self.body, text);
        true
    }

    fn is_comment(&self) -> bool {
        self.body.is_comment
    }

    fn is_whitespace(&self) -> bool {
        self.body.is_whitespace
    }

    fn extend(&mut self, next: &dyn Change) -> bool {
        let count = line_count(&self.body.text);
        extend_following(self, count, count, next)
    }

    fn rebuild(
        &self,
        file: &Path,
        _source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        for (offset, line) in self.body.text.split('\n').enumerate() {
            let line_no =
                self.body.old.checked_add(offset).ok_or_else(|| {
                    format!("line {} is outside {}", self.body.old, file.display())
                })?;
            plan.replace(file, line_no, line.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_adjacent, promote_named_blocks, LineAdded};

    /// An added line at one insertion point. `new` is its line in the new file.
    fn added(new: usize, text: &str, is_comment: bool, is_whitespace: bool) -> LineAdded {
        LineAdded::new(
            "src/main.rs".to_string(),
            1,
            new,
            text.to_string(),
            is_comment,
            is_whitespace,
        )
    }

    #[test]
    fn a_whitespace_line_stays_apart_from_a_line_that_is_not_whitespace() {
        let merged = merge_adjacent(vec![
            Box::new(added(1, "    ", false, true)),
            Box::new(added(2, "fn main() {}", false, false)),
        ]);

        assert_eq!(merged.len(), 2);
        let first = merged[0].line_edit().expect("line edit");
        let second = merged[1].line_edit().expect("line edit");
        assert!(first.is_whitespace());
        assert!(!second.is_whitespace());
        assert_eq!(first.text(), Some("    "));
        assert_eq!(second.text(), Some("fn main() {}"));
    }

    #[test]
    fn adjacent_whitespace_lines_merge_and_keep_the_flag() {
        let merged = merge_adjacent(vec![
            Box::new(added(1, "    ", false, true)),
            Box::new(added(2, "\t", false, true)),
        ]);

        assert_eq!(merged.len(), 1);
        let line = merged[0].line_edit().expect("line edit");
        assert!(line.is_whitespace());
        assert!(!line.is_comment());
        assert_eq!(line.text(), Some("    \n\t"));
    }

    #[test]
    fn a_function_addition_becomes_a_named_block() {
        let merged = promote_named_blocks(vec![Box::new(added(
            2,
            "fn added() {\n    let value = 1;\n}",
            false,
            false,
        ))]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind(), "named-block-added");
        let line = merged[0].line_edit().expect("line edit");
        assert_eq!(line.text(), Some("fn added() {\n    let value = 1;\n}"));
        assert!(!line.is_comment());
        assert!(!line.is_whitespace());
        let mut details = String::new();
        merged[0].write_details(&mut details);
        assert!(details.contains("name: \"added\"\n"), "{details}");
    }

    #[test]
    fn a_comment_or_plain_addition_stays_a_line_addition() {
        let merged = promote_named_blocks(vec![
            Box::new(added(1, "// fn fake() {}", true, false)),
            Box::new(added(2, "let value = 1;", false, false)),
        ]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].kind(), "line-added");
        assert_eq!(merged[1].kind(), "line-added");
    }
}
