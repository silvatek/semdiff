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

use crate::dialect::BlockCategory;
use crate::filesystem::FileSystem;
use crate::semdif::{write_detail, write_plain};

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
    write_plain(document, "old", &body.old.to_string());
    write_plain(document, "lines", &format_lines(lines));
    write_plain(document, "new", &body.new.to_string());
    write_detail(document, "text", &body.text);
    if body.is_comment {
        write_plain(document, "isComment", "true");
    }
    if body.is_whitespace {
        write_plain(document, "isWhitespace", "true");
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
/// [`NamedBlockAdded`] or [`NamedBlockRemoved`]. An indented block that
/// continues on a later record of the same kind is that one block, and the
/// blank lines between those records are part of it. A comment that is still
/// inside the block leaves the records as line additions or removals. Apply
/// inserts or deletes the same lines. A header without its body, a run of two
/// blocks, and a brace block split by a blank line stay line additions or
/// removals. `new_sources` is the new text of each file and `old_sources` is
/// the old text, so a callable inside a type's block can be recorded as a
/// method.
pub(crate) fn promote_named_blocks(
    new_sources: &[super::SourceText],
    old_sources: &[super::SourceText],
    changes: &mut super::ChangeList,
) {
    let mut index = 0;
    while index < changes.len() {
        let last = match block_extent(changes, index) {
            BlockExtent::Held => {
                index += 1;
                continue;
            }
            BlockExtent::Through(last) => last,
        };
        let drop = (index + 1..=last).collect::<Vec<_>>();
        if let Some(promoted) = named_block_from_addition(new_sources, changes, index, last) {
            changes.merge(index, &drop, Box::new(promoted));
            index += 1;
            continue;
        }
        if let Some(promoted) = named_block_from_removal(old_sources, changes, index, last) {
            changes.merge(index, &drop, Box::new(promoted));
            index += 1;
            continue;
        }
        index += 1;
    }
}

/// How many records belong to the block that opens at `index`.
///
/// `Through` names the last record to include. A block that does not continue
/// names its own index. `Held` means a later comment is still inside the
/// block, so the records stay line additions or removals.
enum BlockExtent {
    Through(usize),
    Held,
}

/// The records from `index` that form one indented block.
///
/// A blank line does not end the block and does not answer whether it
/// continues. The next line that is not blank does. A line the dialect still
/// places inside the block is included, and so are the blank lines before it.
/// A comment in that position holds the block: it is not joined, and the
/// opening record is not promoted. A line outside the block stops the walk
/// before any blank line that only separates the two. Braces do not continue
/// here. An unclosed brace block is left as a line addition or removal.
fn block_extent(changes: &super::ChangeList, index: usize) -> BlockExtent {
    let Some(walk) = BlockWalk::start(changes, index) else {
        return BlockExtent::Through(index);
    };
    walk.extent(changes)
}

/// The position and header of one addition or removal while later records are considered.
struct BlockWalk {
    removal: bool,
    scope: String,
    header: String,
    old_at: usize,
    new_at: usize,
    last: usize,
}

impl BlockWalk {
    fn start(changes: &super::ChangeList, index: usize) -> Option<Self> {
        let current = &changes[index];
        let removal = match current.kind() {
            "line-added" => false,
            "line-removed" => true,
            _ => return None,
        };
        let line = current.line_edit()?;
        if line.is_whitespace() || line.is_comment() {
            return None;
        }
        let text = line.text()?.to_string();
        let (old, new) = line.position();
        let (old_at, new_at) = next_position(removal, old, new, &text)?;
        Some(Self {
            removal,
            scope: current.scope().to_string(),
            header: text,
            old_at,
            new_at,
            last: index,
        })
    }

    fn extent(mut self, changes: &super::ChangeList) -> BlockExtent {
        let mut cursor = self.last + 1;
        while cursor < changes.len() {
            let Some(text) = self.following_text(changes, cursor) else {
                break;
            };
            if changes[cursor]
                .line_edit()
                .is_some_and(|line| line.is_whitespace() && !line.is_comment())
            {
                cursor += 1;
                continue;
            }
            if !crate::family::block_resumes(&self.scope, &self.header, &text) {
                break;
            }
            if changes[cursor]
                .line_edit()
                .is_some_and(|line| line.is_comment())
            {
                return BlockExtent::Held;
            }
            self.last = cursor;
            cursor += 1;
        }
        BlockExtent::Through(self.last)
    }

    /// The text of the next same-kind record, when it starts where this block left off.
    fn following_text(&mut self, changes: &super::ChangeList, cursor: usize) -> Option<String> {
        let next = &changes[cursor];
        if next.kind() != kind_name(self.removal) || next.scope() != self.scope {
            return None;
        }
        let line = next.line_edit()?;
        let (next_old, next_new) = line.position();
        if next_old != self.old_at || next_new != self.new_at {
            return None;
        }
        let text = line.text()?.to_string();
        (self.old_at, self.new_at) = next_position(self.removal, self.old_at, self.new_at, &text)?;
        Some(text)
    }
}

fn kind_name(removal: bool) -> &'static str {
    if removal {
        "line-removed"
    } else {
        "line-added"
    }
}

/// The line position just after a record with this text.
///
/// A removal continues down the old file and keeps its new line. An addition
/// continues down the new file and keeps its old line.
fn next_position(removal: bool, old: usize, new: usize, text: &str) -> Option<(usize, usize)> {
    let count = line_count(text);
    if removal {
        Some((old.checked_add(count)?, new))
    } else {
        Some((old, new.checked_add(count)?))
    }
}

/// The named block in records `index` through `last`, when they are one addition.
///
/// `last` includes a continued indented body and the blank lines inside it.
/// The first record supplies the line numbers. The joined text is the block.
fn named_block_from_addition(
    sources: &[super::SourceText],
    changes: &super::ChangeList,
    index: usize,
    last: usize,
) -> Option<NamedBlockAdded> {
    let change = &changes[index];
    if change.kind() != "line-added" {
        return None;
    }
    let line = change.line_edit()?;
    if line.is_comment() || line.is_whitespace() {
        return None;
    }
    let text = joined_block_text(changes, index, last)?;
    let (old, new) = line.position();
    let before = lines_before(sources, change.scope(), new);
    let block = crate::family::named_block_in(change.scope(), &text, &before)?;
    Some(NamedBlockAdded::new(
        block_scope(change.scope(), block.enclosing.as_deref()),
        old,
        new,
        text,
        block.category,
        block.name,
    ))
}

/// The named block in records `index` through `last`, when they are one removal.
///
/// `last` includes a continued indented body and the blank lines inside it.
/// The first record supplies the line numbers. The joined text is the block,
/// read against the old file above it.
fn named_block_from_removal(
    sources: &[super::SourceText],
    changes: &super::ChangeList,
    index: usize,
    last: usize,
) -> Option<NamedBlockRemoved> {
    let change = &changes[index];
    if change.kind() != "line-removed" {
        return None;
    }
    let line = change.line_edit()?;
    if line.is_comment() || line.is_whitespace() {
        return None;
    }
    let text = joined_block_text(changes, index, last)?;
    let (old, new) = line.position();
    let before = lines_before(sources, change.scope(), old);
    let block = crate::family::named_block_in(change.scope(), &text, &before)?;
    Some(NamedBlockRemoved::new(
        block_scope(change.scope(), block.enclosing.as_deref()),
        old,
        new,
        text,
        block.category,
        block.name,
    ))
}

/// The text of records `index` through `last`, joined as lines of one block.
fn joined_block_text(changes: &super::ChangeList, index: usize, last: usize) -> Option<String> {
    let mut text = changes[index].line_edit()?.text()?.to_string();
    for cursor in index + 1..=last {
        text.push('\n');
        text.push_str(changes[cursor].line_edit()?.text()?);
    }
    Some(text)
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

    fn lines_added(&self) -> usize {
        line_count(&self.body.text)
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
    category: BlockCategory,
    name: String,
}

impl NamedBlockAdded {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        category: BlockCategory,
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
        write_detail(document, "category", self.category.as_str());
        write_detail(document, "name", &self.name);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn lines_added(&self) -> usize {
        line_count(&self.body.text)
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
    category: BlockCategory,
    name: String,
}

impl NamedBlockRemoved {
    pub(crate) fn new(
        scope: String,
        old: usize,
        new: usize,
        text: String,
        category: BlockCategory,
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
        write_detail(document, "category", self.category.as_str());
        write_detail(document, "name", &self.name);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn lines_removed(&self) -> usize {
        line_count(&self.body.text)
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

    fn lines_removed(&self) -> usize {
        line_count(&self.body.text)
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
    use super::super::{ChangeList, SourceText};
    use super::{merge_adjacent, promote_named_blocks, LineAdded, LineRemoved};

    /// A change list holding these changes, in order.
    fn listed<T: super::Change + 'static>(items: impl IntoIterator<Item = T>) -> ChangeList {
        let mut changes = ChangeList::new();
        for item in items {
            changes.add(item);
        }
        changes
    }

    /// The list after adjacent line edits have been merged.
    fn adjacent(mut changes: ChangeList) -> ChangeList {
        merge_adjacent(&mut changes);
        changes
    }

    /// The list after line additions and removals have been promoted.
    fn promoted(
        new_sources: &[SourceText],
        old_sources: &[SourceText],
        mut changes: ChangeList,
    ) -> ChangeList {
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
        let merged = adjacent(listed([
            added(1, "    ", false, true),
            added(2, "fn main() {}", false, false),
        ]));

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
        let merged = adjacent(listed([
            added(1, "    ", false, true),
            added(2, "\t", false, true),
        ]));

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
            listed([added(
                2,
                "fn added() {\n    let value = 1;\n}",
                false,
                false,
            )]),
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
        let sources = [SourceText {
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
            listed([added(2, "    fn added() {}", false, false)]),
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
            listed([removed(
                2,
                "fn dropped() {\n    let value = 1;\n}",
                false,
                false,
            )]),
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
        let sources = [SourceText {
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
            listed([removed(2, "    fn dropped() {}", false, false)]),
        );

        assert_eq!(merged[0].kind(), "named-block-removed");
        assert_eq!(merged[0].scope(), "src/main.rs[Foo]");
        assert_eq!(merged[0].content().expect("content").file(), "src/main.rs");
        let mut details = String::new();
        merged[0].write_details(&mut details);
        assert!(details.contains("lines: -1\n"), "{details}");
        assert!(details.contains("category: \"method\"\n"), "{details}");
        assert!(details.contains("name: \"dropped\"\n"), "{details}");

        let nested = [SourceText {
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
            listed([removed(2, "    fn dropped() {}", false, false)]),
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
            listed([removed(
                2,
                "fn dropped() {\n    let value = 1;",
                false,
                false,
            )]),
        );
        assert_eq!(unclosed[0].kind(), "line-removed");

        let pair = promoted(
            &[],
            &[],
            listed([removed(2, "fn one() {}\nfn two() {}", false, false)]),
        );
        assert_eq!(pair[0].kind(), "line-removed");
    }

    #[test]
    fn a_python_removal_split_by_a_blank_line_is_one_block() {
        let continued = promoted(
            &[],
            &[],
            listed([
                LineRemoved::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def dropped():\n    return 1".to_string(),
                    false,
                    false,
                ),
                LineRemoved::new("calc.py".to_string(), 3, 1, "".to_string(), false, true),
                LineRemoved::new(
                    "calc.py".to_string(),
                    4,
                    1,
                    "    return 2".to_string(),
                    false,
                    false,
                ),
            ]),
        );
        assert_eq!(continued.len(), 1);
        assert_eq!(continued[0].kind(), "named-block-removed");
        let mut details = String::new();
        continued[0].write_details(&mut details);
        assert!(details.contains("lines: -4\n"), "{details}");
        assert!(
            details.contains("text: \"def dropped():\\n    return 1\\n\\n    return 2\"\n"),
            "{details}"
        );
        assert!(details.contains("name: \"dropped\"\n"), "{details}");
    }

    #[test]
    fn an_unclosed_or_paired_function_stays_a_line_addition() {
        let unclosed = promoted(
            &[],
            &[],
            listed([added(2, "fn added() {\n    let value = 1;", false, false)]),
        );
        assert_eq!(unclosed[0].kind(), "line-added");

        let pair = promoted(
            &[],
            &[],
            listed([added(2, "fn one() {}\nfn two() {}", false, false)]),
        );
        assert_eq!(pair[0].kind(), "line-added");
    }

    #[test]
    fn a_python_addition_split_by_a_blank_line_is_one_block() {
        let continued = promoted(
            &[],
            &[],
            listed([
                LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def added():\n    return 1".to_string(),
                    false,
                    false,
                ),
                LineAdded::new("calc.py".to_string(), 1, 3, "".to_string(), false, true),
                LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    4,
                    "    return 2".to_string(),
                    false,
                    false,
                ),
            ]),
        );
        assert_eq!(continued.len(), 1);
        assert_eq!(continued[0].kind(), "named-block-added");
        let mut details = String::new();
        continued[0].write_details(&mut details);
        assert!(details.contains("lines: +4\n"), "{details}");
        assert!(
            details.contains("text: \"def added():\\n    return 1\\n\\n    return 2\"\n"),
            "{details}"
        );
        assert!(details.contains("name: \"added\"\n"), "{details}");
    }

    #[test]
    fn a_python_comment_inside_the_block_leaves_the_addition() {
        let commented = promoted(
            &[],
            &[],
            listed([
                LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def added():\n    return 1".to_string(),
                    false,
                    false,
                ),
                LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    3,
                    "    # note".to_string(),
                    true,
                    false,
                ),
            ]),
        );
        assert_eq!(commented[0].kind(), "line-added");
        assert_eq!(commented[1].kind(), "line-added");
    }

    #[test]
    fn a_blank_line_between_python_functions_leaves_each_one() {
        let merged = promoted(
            &[],
            &[],
            listed([
                LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    1,
                    "def one():\n    return 1".to_string(),
                    false,
                    false,
                ),
                LineAdded::new("calc.py".to_string(), 1, 3, "".to_string(), false, true),
                LineAdded::new(
                    "calc.py".to_string(),
                    1,
                    4,
                    "def two():\n    return 2".to_string(),
                    false,
                    false,
                ),
            ]),
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
            listed([
                added(1, "// fn fake() {}", true, false),
                added(2, "let value = 1;", false, false),
            ]),
        );

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].kind(), "line-added");
        assert_eq!(merged[1].kind(), "line-added");
    }
}
