//! Line additions, removals, and modifications.
//!
//! A line change's scope is the file. A span change names one old line instead,
//! and lives in its own module.

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
}

fn line_count(text: &str) -> usize {
    text.split('\n').count()
}

/// Joins `next` onto `current` when it continues the same kind and scope.
///
/// `old_len` and `new_len` are how far `current` already reaches. The next
/// change continues it when each side's first line plus that length equals
/// the next change's first line. The joined text is stored on `current`.
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
    let Some((old, new)) = current.line_edit().map(|line| line.position()) else {
        return false;
    };
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
/// A run of line edits in one scope becomes one change. Removals follow one
/// another on the old side and share the new line that follows them, so
/// `lines` is `-1` or `-2`. Additions share an insertion point and follow one
/// another on the new side, so `lines` is `+1` or `+2`. Modifications follow
/// one another on both sides. They do not change the line count, so `lines`
/// stays `+0`. The merged change keeps the first line number. Its text keeps
/// every line, joined by a newline. A span change stays its own change.
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

pub(crate) struct LineAdded {
    scope: String,
    body: LineBody,
}

impl LineAdded {
    pub(crate) fn new(scope: String, old: usize, new: usize, text: String) -> Self {
        Self {
            scope,
            body: LineBody { old, new, text },
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

pub(crate) struct LineRemoved {
    scope: String,
    body: LineBody,
}

impl LineRemoved {
    pub(crate) fn new(scope: String, old: usize, new: usize, text: String) -> Self {
        Self {
            scope,
            body: LineBody { old, new, text },
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
    pub(crate) fn new(scope: String, old: usize, new: usize, text: String) -> Self {
        Self {
            scope,
            body: LineBody { old, new, text },
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
