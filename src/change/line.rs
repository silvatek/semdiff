//! Line additions, removals, modifications, and identifier renames.

use std::path::Path;

use super::{applied_with_its_file, Change, Content, LineEdit, LinePlan, LineStart};

/// The line numbers and text shared by a line addition, removal, and modification.
///
/// The scope stays on each of those changes. These three differ in how long
/// the edit is on the old side and the new side, and in whether a following
/// edit of the same kind can be appended. An identifier rename does not use
/// this. It also stores the two names, and it stays its own change.
struct LineBody {
    old: usize,
    new: usize,
    text: String,
}

fn line_count(text: &str) -> usize {
    text.split('\n').count()
}

/// Joins `next` onto a line edit when it continues the same kind and scope.
///
/// `old_len` and `new_len` are how far this edit already reaches. The next
/// change continues it when each side's first line plus that length equals
/// the next change's first line.
fn extend_following(
    kind: &str,
    scope: &str,
    body: &mut LineBody,
    old_len: usize,
    new_len: usize,
    next: &dyn Change,
) -> bool {
    if next.kind() != kind || next.scope() != scope {
        return false;
    }
    let Some(line) = next.line_edit() else {
        return false;
    };
    let Some(following) = line.start() else {
        return false;
    };
    let Some(next_old) = body.old.checked_add(old_len) else {
        return false;
    };
    let Some(next_new) = body.new.checked_add(new_len) else {
        return false;
    };
    if next_old != following.old || next_new != following.new {
        return false;
    }
    let Some(text) = line.text() else {
        return false;
    };
    body.text.push('\n');
    body.text.push_str(text);
    true
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
/// every line, joined by a newline. An identifier rename stays its own change.
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

    fn apply(&self, _source: &Path) -> Result<(), String> {
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

    fn extend(&mut self, next: &dyn Change) -> bool {
        let new_len = line_count(&self.body.text);
        extend_following(self.kind(), &self.scope, &mut self.body, 0, new_len, next)
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

    fn apply(&self, _source: &Path) -> Result<(), String> {
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

    fn extend(&mut self, next: &dyn Change) -> bool {
        let old_len = line_count(&self.body.text);
        extend_following(self.kind(), &self.scope, &mut self.body, old_len, 0, next)
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

    fn apply(&self, _source: &Path) -> Result<(), String> {
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

    fn extend(&mut self, next: &dyn Change) -> bool {
        let count = line_count(&self.body.text);
        extend_following(self.kind(), &self.scope, &mut self.body, count, count, next)
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

pub(crate) struct IdentifierRenamed {
    pub(crate) scope: String,
    pub(crate) old: usize,
    pub(crate) new: usize,
    pub(crate) oldname: String,
    pub(crate) newname: String,
}

impl Change for IdentifierRenamed {
    fn kind(&self) -> &'static str {
        "identifier-renamed"
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_plain(document, "old", &self.old.to_string());
        crate::semdif::write_plain(document, "new", &self.new.to_string());
        crate::semdif::write_detail(document, "oldname", &self.oldname);
        crate::semdif::write_detail(document, "newname", &self.newname);
    }

    fn apply(&self, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for IdentifierRenamed {
    fn file(&self) -> &str {
        &self.scope
    }
}

impl LineEdit for IdentifierRenamed {
    fn position(&self) -> (usize, usize) {
        (self.old, self.new)
    }

    fn rebuild(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        if self.old == 0 || self.old > source_lines.len() {
            return Err(format!(
                "line {} is outside {} which has {} lines",
                self.old,
                file.display(),
                source_lines.len()
            ));
        }
        let replaced = match crate::identifier::replace_name(
            &self.scope,
            &source_lines[self.old - 1],
            &self.oldname,
            &self.newname,
        ) {
            Ok(text) => text,
            Err(message) => {
                return Err(format!(
                    "cannot rename {} on line {} in {}: {message}",
                    self.oldname,
                    self.old,
                    file.display()
                ));
            }
        };
        plan.replace(file, self.old, replaced)
    }
}
