//! The record of a line-level change, including how such records combine and how
//! apply carries one out.
//!
//! A line change's scope is the file. A method's scope also names the enclosing
//! block, as `src/main.rs[Count]`. Apply still edits the file. A span change
//! names one old line instead, and lives in its own module. The same line
//! record can say that the edit is a full-line comment, and that it is
//! insignificant whitespace. Those are `isComment: true` and
//! `isWhitespace: true` in the semdif. False is the default and is left out.
//! A record may carry both. An addition the dialect reads as a new function,
//! method, or structure is `named-block-added`, and a removal of one is
//! `named-block-removed`. Each record carries that block's name.

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
pub(crate) fn merge_adjacent(changes: &mut super::ChangeList) {
    let mut index = 0;
    while index < changes.len() {
        let absorbed = changes
            .with_next(index, |current, next| match current.line_edit_mut() {
                Some(line) => line.extend(next),
                None => false,
            })
            .unwrap_or(false);
        if absorbed {
            changes.remove(index + 1);
        } else {
            index += 1;
        }
    }
}

/// Records a line addition or removal as a named block when it is one complete block.
///
/// Merging has already joined adjacent lines of one kind into one record. A
/// blank line stays its own record, and so does a comment. This pass reads
/// one addition or removal and, when its first line opens a function, method,
/// or structure and the text is that one block, replaces it with
/// [`NamedBlockAdded`] or [`NamedBlockRemoved`]. The line text is unchanged,
/// so apply inserts or deletes the same lines. A header without its body, a
/// run of two blocks, and a block that continues on a following record stay
/// line additions or removals. `new_sources` is the new text of each file and
/// `old_sources` is the old text, so a callable inside a type's block can be
/// recorded as a method.
pub(crate) fn promote_named_blocks(
    new_sources: &[super::SourceText],
    old_sources: &[super::SourceText],
    changes: &mut super::ChangeList,
) {
    for index in 0..changes.len() {
        if followed_by_more_of_the_block(changes, index) {
            continue;
        }
        let added = named_block_from_addition(new_sources, &changes[index]);
        if let Some(promoted) = added {
            changes.replace(index, Box::new(promoted));
            continue;
        }
        let removed = named_block_from_removal(old_sources, &changes[index]);
        if let Some(promoted) = removed {
            changes.replace(index, Box::new(promoted));
        }
    }
}

/// Whether a later record is still inside the block opened by `changes[index]`.
///
/// Blank lines between the two are skipped, because a blank line does not end
/// an indented block and does not join the record. The next line that is not
/// blank answers. An addition continues on the new side. A removal continues
/// on the old side. Braces do not use this: an unclosed brace block is
/// already left as a line addition or removal.
fn followed_by_more_of_the_block(changes: &super::ChangeList, index: usize) -> bool {
    let current = &changes[index];
    let removal = match current.kind() {
        "line-added" => false,
        "line-removed" => true,
        _ => return false,
    };
    let Some(line) = current.line_edit() else {
        return false;
    };
    if line.is_whitespace() || line.is_comment() {
        return false;
    }
    let Some(text) = line.text() else {
        return false;
    };
    let (old, new) = line.position();
    let (mut old_at, mut new_at) = if removal {
        let Some(next_old) = old.checked_add(line_count(text)) else {
            return false;
        };
        (next_old, new)
    } else {
        let Some(next_new) = new.checked_add(line_count(text)) else {
            return false;
        };
        (old, next_new)
    };
    let mut cursor = index + 1;
    while cursor < changes.len() {
        let next = &changes[cursor];
        if next.kind() != current.kind() || next.scope() != current.scope() {
            return false;
        }
        let Some(next_line) = next.line_edit() else {
            return false;
        };
        let (next_old, next_new) = next_line.position();
        if next_old != old_at || next_new != new_at {
            return false;
        }
        let Some(next_text) = next_line.text() else {
            return false;
        };
        if next_line.is_whitespace() && !next_line.is_comment() {
            if removal {
                let Some(advanced) = old_at.checked_add(line_count(next_text)) else {
                    return false;
                };
                old_at = advanced;
            } else {
                let Some(advanced) = new_at.checked_add(line_count(next_text)) else {
                    return false;
                };
                new_at = advanced;
            }
            cursor += 1;
            continue;
        }
        return crate::family::block_resumes(current.scope(), text, next_text);
    }
    false
}

fn named_block_from_addition(
    sources: &[super::SourceText],
    change: &dyn super::Change,
) -> Option<NamedBlockAdded> {
    if change.kind() != "line-added" {
        return None;
    }
    let line = change.line_edit()?;
    if line.is_comment() || line.is_whitespace() {
        return None;
    }
    let text = line.text()?;
    let (old, new) = line.position();
    let before = lines_before(sources, change.scope(), new);
    let block = crate::family::named_block_in(change.scope(), text, &before)?;
    Some(NamedBlockAdded::new(
        block_scope(change.scope(), block.enclosing.as_deref()),
        old,
        new,
        text.to_string(),
        block.category,
        block.name,
    ))
}

fn named_block_from_removal(
    sources: &[super::SourceText],
    change: &dyn super::Change,
) -> Option<NamedBlockRemoved> {
    if change.kind() != "line-removed" {
        return None;
    }
    let line = change.line_edit()?;
    if line.is_comment() || line.is_whitespace() {
        return None;
    }
    let text = line.text()?;
    let (old, new) = line.position();
    let before = lines_before(sources, change.scope(), old);
    let block = crate::family::named_block_in(change.scope(), text, &before)?;
    Some(NamedBlockRemoved::new(
        block_scope(change.scope(), block.enclosing.as_deref()),
        old,
        new,
        text.to_string(),
        block.category,
        block.name,
    ))
}

/// The scope written for a named block.
///
/// A method names the enclosing block in brackets, as `src/main.rs[Count]`.
/// Any other block names only the file.
fn block_scope(path: &str, enclosing: Option<&str>) -> String {
    match enclosing {
        Some(name) => format!("{path}[{name}]"),
        None => path.to_string(),
    }
}

/// The file a scope names.
///
/// A method's scope is `path[Block]`. Apply edits `path`. Any other scope is
/// already the file.
fn scope_file(scope: &str) -> &str {
    let Some(open) = scope.rfind('[') else {
        return scope;
    };
    let Some(inside) = scope[open + 1..].strip_suffix(']') else {
        return scope;
    };
    if open == 0 || inside.is_empty() || inside.contains('[') {
        return scope;
    }
    &scope[..open]
}

/// The file above line `line`, joined the way the record is joined.
///
/// `line` is 1-based. Line 1 has nothing above it. A file that is not in
/// `sources` has nothing above it either, so the block is read at the top.
/// An addition uses the new file and its new line. A removal uses the old
/// file and its old line.
fn lines_before(sources: &[super::SourceText], path: &str, line: usize) -> String {
    let Some(source) = sources.iter().find(|source| source.path == path) else {
        return String::new();
    };
    let end = line.saturating_sub(1).min(source.lines.len());
    source.lines[..end].join("\n")
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
/// `category` is the shared kind of block, and `name` is the identifier that
/// follows the dialect's keyword. A method's scope is the file and the
/// enclosing block, as `src/main.rs[Count]`. The record keeps the added
/// lines, and apply inserts them into the file. It does not join a
/// neighboring edit: the name belongs to this one block.
pub(crate) struct NamedBlockAdded {
    scope: String,
    body: LineBody,
    category: crate::dialect::BlockCategory,
    name: String,
}

impl NamedBlockAdded {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        category: crate::dialect::BlockCategory,
        name: String,
    ) -> Self {
        Self {
            scope,
            body: LineBody {
                old,
                new,
                text,
                is_comment: false,
                is_whitespace: false,
            },
            category,
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
        crate::semdif::write_detail(document, "category", self.category.as_str());
        crate::semdif::write_detail(document, "name", &self.name);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for NamedBlockAdded {
    fn file(&self) -> &str {
        scope_file(&self.scope)
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

/// A line removal that deletes a named block.
///
/// `category` is the shared kind of block, and `name` is the identifier that
/// follows the dialect's keyword. A method's scope is the file and the
/// enclosing block, as `src/main.rs[Count]`, read from the old file. The
/// record keeps the removed lines, and apply deletes them from the file. It
/// does not join a neighboring edit: the name belongs to this one block.
pub(crate) struct NamedBlockRemoved {
    scope: String,
    body: LineBody,
    category: crate::dialect::BlockCategory,
    name: String,
}

impl NamedBlockRemoved {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        category: crate::dialect::BlockCategory,
        name: String,
    ) -> Self {
        Self {
            scope,
            body: LineBody {
                old,
                new,
                text,
                is_comment: false,
                is_whitespace: false,
            },
            category,
            name,
        }
    }
}

impl Change for NamedBlockRemoved {
    fn kind(&self) -> &'static str {
        "named-block-removed"
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
        crate::semdif::write_detail(document, "category", self.category.as_str());
        crate::semdif::write_detail(document, "name", &self.name);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for NamedBlockRemoved {
    fn file(&self) -> &str {
        scope_file(&self.scope)
    }
}

impl LineEdit for NamedBlockRemoved {
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
        let count = line_count(&self.body.text);
        let old_end = self
            .body
            .old
            .checked_add(count - 1)
            .ok_or_else(|| format!("line {} is outside {}", self.body.old, file.display()))?;
        plan.remove_range(file, self.body.old, old_end)
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
    use super::{merge_adjacent, promote_named_blocks, LineAdded, LineRemoved};

    /// The list after adjacent line edits have been merged.
    fn adjacent(records: Vec<Box<dyn super::Change>>) -> crate::change::ChangeList {
        let mut changes = crate::change::ChangeList::of(records);
        merge_adjacent(&mut changes);
        changes
    }

    /// The list after line additions and removals have been promoted.
    fn promoted(
        new_sources: &[crate::change::SourceText],
        old_sources: &[crate::change::SourceText],
        records: Vec<Box<dyn super::Change>>,
    ) -> crate::change::ChangeList {
        let mut changes = crate::change::ChangeList::of(records);
        promote_named_blocks(new_sources, old_sources, &mut changes);
        changes
    }

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

    /// A removed line. `old` is its line in the old file.
    fn removed(old: usize, text: &str, is_comment: bool, is_whitespace: bool) -> LineRemoved {
        LineRemoved::new(
            "src/main.rs".to_string(),
            old,
            old,
            text.to_string(),
            is_comment,
            is_whitespace,
        )
    }

    #[test]
    fn a_whitespace_line_stays_apart_from_a_line_that_is_not_whitespace() {
        let merged = adjacent(vec![
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
        let merged = adjacent(vec![
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
        let merged = promoted(
            &[],
            &[],
            vec![Box::new(added(
                2,
                "fn added() {\n    let value = 1;\n}",
                false,
                false,
            ))],
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind(), "named-block-added");
        let line = merged[0].line_edit().expect("line edit");
        assert_eq!(line.text(), Some("fn added() {\n    let value = 1;\n}"));
        assert!(!line.is_comment());
        assert!(!line.is_whitespace());
        let mut details = String::new();
        merged[0].write_details(&mut details);
        assert!(details.contains("category: \"function\"\n"), "{details}");
        assert!(details.contains("name: \"added\"\n"), "{details}");
    }

    #[test]
    fn a_function_inside_the_new_file_type_becomes_a_method() {
        let sources = [crate::change::SourceText {
            path: "src/main.rs".to_string(),
            lines: vec![
                "impl Foo {".to_string(),
                "    fn added() {}".to_string(),
                "}".to_string(),
            ],
        }];
        let merged = promoted(
            &sources,
            &[],
            vec![Box::new(added(2, "    fn added() {}", false, false))],
        );

        assert_eq!(merged[0].kind(), "named-block-added");
        assert_eq!(merged[0].scope(), "src/main.rs[Foo]");
        assert_eq!(merged[0].content().expect("content").file(), "src/main.rs");
        let mut details = String::new();
        merged[0].write_details(&mut details);
        assert!(details.contains("category: \"method\"\n"), "{details}");
        assert!(details.contains("name: \"added\"\n"), "{details}");
    }

    #[test]
    fn a_function_removal_becomes_a_named_block() {
        let merged = promoted(
            &[],
            &[],
            vec![Box::new(removed(
                2,
                "fn dropped() {\n    let value = 1;\n}",
                false,
                false,
            ))],
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].kind(), "named-block-removed");
        assert_eq!(merged[0].scope(), "src/main.rs");
        let mut details = String::new();
        merged[0].write_details(&mut details);
        assert!(details.contains("lines: -3\n"), "{details}");
        assert!(details.contains("category: \"function\"\n"), "{details}");
        assert!(details.contains("name: \"dropped\"\n"), "{details}");
    }

    #[test]
    fn a_method_removal_inside_the_old_file_type_names_that_type() {
        let sources = [crate::change::SourceText {
            path: "src/main.rs".to_string(),
            lines: vec![
                "impl Foo {".to_string(),
                "    fn dropped() {}".to_string(),
                "}".to_string(),
            ],
        }];
        let merged = promoted(
            &[],
            &sources,
            vec![Box::new(removed(2, "    fn dropped() {}", false, false))],
        );

        assert_eq!(merged[0].kind(), "named-block-removed");
        assert_eq!(merged[0].scope(), "src/main.rs[Foo]");
        assert_eq!(merged[0].content().expect("content").file(), "src/main.rs");
        let mut details = String::new();
        merged[0].write_details(&mut details);
        assert!(details.contains("lines: -1\n"), "{details}");
        assert!(details.contains("category: \"method\"\n"), "{details}");
        assert!(details.contains("name: \"dropped\"\n"), "{details}");

        let nested = [crate::change::SourceText {
            path: "src/main.rs".to_string(),
            lines: vec![
                "fn outer() {".to_string(),
                "    fn dropped() {}".to_string(),
                "    let value = 1;".to_string(),
                "}".to_string(),
            ],
        }];
        let inner = promoted(
            &[],
            &nested,
            vec![Box::new(removed(2, "    fn dropped() {}", false, false))],
        );
        assert_eq!(inner[0].kind(), "named-block-removed");
        assert_eq!(inner[0].scope(), "src/main.rs");
        let mut details = String::new();
        inner[0].write_details(&mut details);
        assert!(details.contains("category: \"function\"\n"), "{details}");
    }

    #[test]
    fn an_unclosed_or_paired_function_stays_a_line_removal() {
        let unclosed = promoted(
            &[],
            &[],
            vec![Box::new(removed(
                2,
                "fn dropped() {\n    let value = 1;",
                false,
                false,
            ))],
        );
        assert_eq!(unclosed[0].kind(), "line-removed");

        let pair = promoted(
            &[],
            &[],
            vec![Box::new(removed(
                2,
                "fn one() {}\nfn two() {}",
                false,
                false,
            ))],
        );
        assert_eq!(pair[0].kind(), "line-removed");
    }

    #[test]
    fn a_python_block_that_continues_after_the_removal_stays_a_line_removal() {
        let continued = promoted(
            &[],
            &[],
            vec![
                Box::new(LineRemoved::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def dropped():\n    return 1".to_string(),
                    false,
                    false,
                )),
                Box::new(LineRemoved::new(
                    "calc.py".to_string(),
                    3,
                    1,
                    "".to_string(),
                    false,
                    true,
                )),
                Box::new(LineRemoved::new(
                    "calc.py".to_string(),
                    4,
                    1,
                    "    return 2".to_string(),
                    false,
                    false,
                )),
            ],
        );
        assert_eq!(continued[0].kind(), "line-removed");
        assert!(continued[1].line_edit().expect("blank").is_whitespace());
        assert_eq!(continued[2].kind(), "line-removed");
    }

    #[test]
    fn an_unclosed_or_paired_function_stays_a_line_addition() {
        let unclosed = promoted(
            &[],
            &[],
            vec![Box::new(added(
                2,
                "fn added() {\n    let value = 1;",
                false,
                false,
            ))],
        );
        assert_eq!(unclosed[0].kind(), "line-added");

        let pair = promoted(
            &[],
            &[],
            vec![Box::new(added(2, "fn one() {}\nfn two() {}", false, false))],
        );
        assert_eq!(pair[0].kind(), "line-added");
    }

    #[test]
    fn a_python_block_that_continues_after_the_addition_stays_a_line_addition() {
        let continued = promoted(
            &[],
            &[],
            vec![
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def added():\n    return 1".to_string(),
                    false,
                    false,
                )),
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    3,
                    "".to_string(),
                    false,
                    true,
                )),
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    4,
                    "    return 2".to_string(),
                    false,
                    false,
                )),
            ],
        );
        assert_eq!(continued[0].kind(), "line-added");
        assert!(continued[1].line_edit().expect("blank").is_whitespace());
        assert_eq!(continued[2].kind(), "line-added");

        let commented = promoted(
            &[],
            &[],
            vec![
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def added():\n    return 1".to_string(),
                    false,
                    false,
                )),
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    3,
                    "    # note".to_string(),
                    true,
                    false,
                )),
            ],
        );
        assert_eq!(commented[0].kind(), "line-added");
        assert_eq!(commented[1].kind(), "line-added");
    }

    #[test]
    fn a_blank_line_between_python_functions_leaves_each_one() {
        let merged = promoted(
            &[],
            &[],
            vec![
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def one():\n    return 1".to_string(),
                    false,
                    false,
                )),
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    3,
                    "".to_string(),
                    false,
                    true,
                )),
                Box::new(LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    4,
                    "def two():\n    return 2".to_string(),
                    false,
                    false,
                )),
            ],
        );

        assert_eq!(merged[0].kind(), "named-block-added");
        assert!(merged[1].line_edit().expect("blank").is_whitespace());
        assert_eq!(merged[2].kind(), "named-block-added");
    }

    #[test]
    fn a_comment_or_plain_addition_stays_a_line_addition() {
        let merged = promoted(
            &[],
            &[],
            vec![
                Box::new(added(1, "// fn fake() {}", true, false)),
                Box::new(added(2, "let value = 1;", false, false)),
            ],
        );

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].kind(), "line-added");
        assert_eq!(merged[1].kind(), "line-added");
    }
}
