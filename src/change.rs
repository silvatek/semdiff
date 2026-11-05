//! One record in a semdif file.
//!
//! Each kind of change is its own type and implements [`Record`]. The drivers
//! that write, apply, sort, and merge ask the record instead of matching on
//! every kind. A new kind registers a parser and implements [`Record`]; those
//! drivers stay as they are.
//!
//! [`Record::apply`] is required, so a kind that does not say how to replay
//! itself does not compile. [`Record::extend`] leaves the record separate
//! unless that kind joins an adjacent one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A change a semdif file can record.
///
/// The value is a trait object so the list can hold every kind. The concrete
/// type supplies the fields and the operations that differ.
pub(crate) type Change = Box<dyn Record>;

/// What every change kind can be asked.
///
/// Methods that only some kinds take part in have a default. A line removal
/// overrides [`Record::extend`]. A directory addition does not, and stays its
/// own record.
pub(crate) trait Record {
    /// The semdif type name, such as `line-removed`.
    fn kind(&self) -> &'static str;

    /// The path this record is about.
    ///
    /// A rename reports the path it moves from. Sorting uses this path unless
    /// [`Record::sort_key`] says otherwise.
    fn path(&self) -> &str;

    /// The scope written on the record.
    ///
    /// A content edit is scoped to its file. A change to the tree is scoped
    /// to `.`.
    fn scope(&self) -> &str {
        self.path()
    }

    /// The pair used to order records before they are written.
    fn sort_key(&self) -> (&str, &str) {
        (self.path(), "")
    }

    /// The file whose contents this record edits, when it is applied with the
    /// other edits of that file.
    fn content_path(&self) -> Option<&str> {
        None
    }

    /// Writes the detail lines, under a `details` key the driver already opened.
    fn write_details(&self, document: &mut String);

    /// Replays this record on its own.
    ///
    /// A content edit is replayed with the other edits of its file, so its
    /// implementation reports that instead of changing anything.
    fn apply(&self, source: &Path) -> Result<(), String>;

    /// Joins `next` onto this record when the two are one adjacent run.
    ///
    /// The default keeps them separate. A kind that merges checks the kind,
    /// the file, and its own positions, then appends the next record's text.
    fn extend(&mut self, _next: &dyn Record) -> bool {
        false
    }

    /// `old`, then `new`, when this record sits on a line.
    fn line_position(&self) -> Option<(usize, usize)> {
        None
    }

    /// This record rewrites lines of its file.
    fn edits_lines(&self) -> bool {
        false
    }

    /// The old and new spans of a line edit, when this record is one.
    fn line_span(&self) -> Option<LineSpan> {
        None
    }

    /// The line text a following record would append, when this record has any.
    fn line_text(&self) -> Option<&str> {
        None
    }

    /// The old and new SHA-256 hex hashes, when this record carries them.
    fn file_digest(&self) -> Option<(&str, &str)> {
        None
    }

    /// The bytes that replace a whole file, when this record carries them.
    fn replacement(&self) -> Option<&[u8]> {
        None
    }

    /// Adds this record's line edit to `plan`.
    fn rebuild_line(
        &self,
        _file: &Path,
        _source_lines: &[String],
        _plan: &mut LinePlan,
    ) -> Result<(), String> {
        Ok(())
    }

    /// The directory this record removes, when it removes one.
    fn removed_directory(&self) -> Option<&str> {
        None
    }

    /// The file this record adds, when adding that file must wait for a
    /// directory at the same path to be removed.
    fn added_file_path(&self) -> Option<&str> {
        None
    }
}

/// The first old line and the first new line of a line edit.
///
/// The length on each side is known by the record being extended. A removal
/// has a new length of zero, so `new` is the line that follows it. An addition
/// has an old length of zero, so `old` is the line it is inserted before.
#[derive(Clone, Copy)]
pub(crate) struct LineSpan {
    old: usize,
    new: usize,
}

/// The line edits collected for one file, before it is rebuilt.
pub(crate) struct LinePlan {
    modified: BTreeMap<usize, String>,
    removed: BTreeSet<usize>,
    added: Vec<(usize, usize, String)>,
}

impl LinePlan {
    pub(crate) fn new() -> Self {
        Self {
            modified: BTreeMap::new(),
            removed: BTreeSet::new(),
            added: Vec::new(),
        }
    }

    fn replace(&mut self, file: &Path, line_no: usize, text: String) -> Result<(), String> {
        if self.modified.insert(line_no, text).is_some() {
            return Err(format!(
                "line {line_no} is modified more than once in {}",
                file.display()
            ));
        }
        Ok(())
    }

    fn remove_range(&mut self, file: &Path, old: usize, old_end: usize) -> Result<(), String> {
        if old_end < old {
            return Err(format!(
                "line {old}-{old_end} is not a removal range in {}",
                file.display()
            ));
        }
        for line in old..=old_end {
            if !self.removed.insert(line) {
                return Err(format!(
                    "line {line} is removed more than once in {}",
                    file.display()
                ));
            }
        }
        Ok(())
    }

    fn insert(&mut self, file: &Path, old: usize, new: usize, text: &str) -> Result<(), String> {
        for (offset, line) in text.split('\n').enumerate() {
            let line_no = new
                .checked_add(offset)
                .ok_or_else(|| format!("cannot insert before line {old} in {}", file.display()))?;
            self.added.push((old, line_no, line.to_string()));
        }
        Ok(())
    }

    /// The file's lines after every edit in this plan.
    pub(crate) fn rebuild(self, file: &Path, lines: &[String]) -> Result<Vec<String>, String> {
        let Self {
            modified,
            removed,
            mut added,
        } = self;
        added.sort_by_key(|(old, new, _)| (*old, *new));
        let count = lines.len();
        for old in modified.keys().chain(removed.iter()) {
            if *old == 0 || *old > count {
                return Err(format!(
                    "line {old} is outside {} which has {count} lines",
                    file.display()
                ));
            }
        }
        for (old, _, _) in &added {
            if *old == 0 || *old > count + 1 {
                return Err(format!(
                    "cannot insert before line {old} in {}: file has {count} lines",
                    file.display()
                ));
            }
        }
        for old in modified.keys() {
            if removed.contains(old) {
                return Err(format!(
                    "line {old} is both modified and removed in {}",
                    file.display()
                ));
            }
        }

        let mut rebuilt = Vec::new();
        for old in 1..=count {
            for (_, _, text) in added.iter().filter(|(before, _, _)| *before == old) {
                rebuilt.push(text.clone());
            }
            if removed.contains(&old) {
                continue;
            }
            if let Some(text) = modified.get(&old) {
                rebuilt.push(text.clone());
            } else {
                rebuilt.push(lines[old - 1].clone());
            }
        }
        for (_, _, text) in added.iter().filter(|(before, _, _)| *before == count + 1) {
            rebuilt.push(text.clone());
        }
        Ok(rebuilt)
    }
}

struct LineBody {
    path: String,
    old: usize,
    new: usize,
    text: String,
}

fn line_count(text: &str) -> usize {
    text.split('\n').count()
}

/// Joins `next` onto a line edit when it continues the same kind and file.
///
/// `old_len` and `new_len` are the span already stored. The next record
/// continues the run when each side's start plus that length equals the next
/// start.
fn extend_span(
    kind: &str,
    body: &mut LineBody,
    old_len: usize,
    new_len: usize,
    next: &dyn Record,
) -> bool {
    if next.kind() != kind || next.scope() != body.path {
        return false;
    }
    let Some(following) = next.line_span() else {
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
    let Some(text) = next.line_text() else {
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

fn applied_with_its_file(kind: &str) -> Result<(), String> {
    Err(format!("{kind} is applied with its file"))
}

fn span(body: &LineBody) -> LineSpan {
    LineSpan {
        old: body.old,
        new: body.new,
    }
}

pub(crate) struct FileRenamed {
    pub(crate) from: String,
    pub(crate) to: String,
}

impl Record for FileRenamed {
    fn kind(&self) -> &'static str {
        "file-renamed"
    }

    fn path(&self) -> &str {
        &self.from
    }

    fn scope(&self) -> &str {
        "."
    }

    fn sort_key(&self) -> (&str, &str) {
        (&self.from, &self.to)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "from", &self.from);
        crate::semdif::write_detail(document, "to", &self.to);
    }

    fn apply(&self, source: &Path) -> Result<(), String> {
        crate::apply::rename_file(source, &self.from, &self.to)
    }
}

pub(crate) struct FileDeleted {
    pub(crate) path: String,
}

impl Record for FileDeleted {
    fn kind(&self) -> &'static str {
        "file-deleted"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn scope(&self) -> &str {
        "."
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
    }

    fn apply(&self, source: &Path) -> Result<(), String> {
        crate::apply::delete_file(source, &self.path)
    }
}

pub(crate) struct FileAdded {
    pub(crate) path: String,
    pub(crate) contents: Vec<u8>,
}

impl Record for FileAdded {
    fn kind(&self) -> &'static str {
        "file-added"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn scope(&self) -> &str {
        "."
    }

    fn added_file_path(&self) -> Option<&str> {
        Some(&self.path)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
        crate::semdif::write_file_contents(document, &self.contents);
    }

    fn apply(&self, source: &Path) -> Result<(), String> {
        crate::apply::write_file(source, &self.path, &self.contents)
    }
}

pub(crate) struct FileChanged {
    pub(crate) path: String,
    pub(crate) contents: Vec<u8>,
}

impl Record for FileChanged {
    fn kind(&self) -> &'static str {
        "file-changed"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn content_path(&self) -> Option<&str> {
        Some(&self.path)
    }

    fn replacement(&self) -> Option<&[u8]> {
        Some(&self.contents)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_file_contents(document, &self.contents);
    }

    fn apply(&self, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

pub(crate) struct FileDigest {
    pub(crate) path: String,
    pub(crate) old_hash: String,
    pub(crate) new_hash: String,
}

impl Record for FileDigest {
    fn kind(&self) -> &'static str {
        "file-changed"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn content_path(&self) -> Option<&str> {
        Some(&self.path)
    }

    fn file_digest(&self) -> Option<(&str, &str)> {
        Some((&self.old_hash, &self.new_hash))
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "old", &self.old_hash);
        crate::semdif::write_detail(document, "new", &self.new_hash);
    }

    fn apply(&self, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

pub(crate) struct LineAdded {
    body: LineBody,
}

impl LineAdded {
    pub(crate) fn new(path: String, old: usize, new: usize, text: String) -> Self {
        Self {
            body: LineBody {
                path,
                old,
                new,
                text,
            },
        }
    }
}

impl Record for LineAdded {
    fn kind(&self) -> &'static str {
        "line-added"
    }

    fn path(&self) -> &str {
        &self.body.path
    }

    fn content_path(&self) -> Option<&str> {
        Some(&self.body.path)
    }

    fn line_position(&self) -> Option<(usize, usize)> {
        Some((self.body.old, self.body.new))
    }

    fn edits_lines(&self) -> bool {
        true
    }

    fn line_span(&self) -> Option<LineSpan> {
        Some(span(&self.body))
    }

    fn line_text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn write_details(&self, document: &mut String) {
        write_line(document, &self.body, line_count(&self.body.text) as isize);
    }

    fn apply(&self, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn extend(&mut self, next: &dyn Record) -> bool {
        let new_len = line_count(&self.body.text);
        extend_span(self.kind(), &mut self.body, 0, new_len, next)
    }

    fn rebuild_line(
        &self,
        file: &Path,
        _source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        plan.insert(file, self.body.old, self.body.new, &self.body.text)
    }
}

pub(crate) struct LineRemoved {
    body: LineBody,
}

impl LineRemoved {
    pub(crate) fn new(path: String, old: usize, new: usize, text: String) -> Self {
        Self {
            body: LineBody {
                path,
                old,
                new,
                text,
            },
        }
    }
}

impl Record for LineRemoved {
    fn kind(&self) -> &'static str {
        "line-removed"
    }

    fn path(&self) -> &str {
        &self.body.path
    }

    fn content_path(&self) -> Option<&str> {
        Some(&self.body.path)
    }

    fn line_position(&self) -> Option<(usize, usize)> {
        Some((self.body.old, self.body.new))
    }

    fn edits_lines(&self) -> bool {
        true
    }

    fn line_span(&self) -> Option<LineSpan> {
        Some(span(&self.body))
    }

    fn line_text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn write_details(&self, document: &mut String) {
        let count = line_count(&self.body.text) as isize;
        write_line(document, &self.body, -count);
    }

    fn apply(&self, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn extend(&mut self, next: &dyn Record) -> bool {
        let old_len = line_count(&self.body.text);
        extend_span(self.kind(), &mut self.body, old_len, 0, next)
    }

    fn rebuild_line(
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
    body: LineBody,
}

impl LineModified {
    pub(crate) fn new(path: String, old: usize, new: usize, text: String) -> Self {
        Self {
            body: LineBody {
                path,
                old,
                new,
                text,
            },
        }
    }
}

impl Record for LineModified {
    fn kind(&self) -> &'static str {
        "line-modified"
    }

    fn path(&self) -> &str {
        &self.body.path
    }

    fn content_path(&self) -> Option<&str> {
        Some(&self.body.path)
    }

    fn line_position(&self) -> Option<(usize, usize)> {
        Some((self.body.old, self.body.new))
    }

    fn edits_lines(&self) -> bool {
        true
    }

    fn line_span(&self) -> Option<LineSpan> {
        Some(span(&self.body))
    }

    fn line_text(&self) -> Option<&str> {
        Some(&self.body.text)
    }

    fn write_details(&self, document: &mut String) {
        write_line(document, &self.body, 0);
    }

    fn apply(&self, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn extend(&mut self, next: &dyn Record) -> bool {
        let count = line_count(&self.body.text);
        extend_span(self.kind(), &mut self.body, count, count, next)
    }

    fn rebuild_line(
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
    pub(crate) path: String,
    pub(crate) old: usize,
    pub(crate) new: usize,
    pub(crate) oldname: String,
    pub(crate) newname: String,
}

impl Record for IdentifierRenamed {
    fn kind(&self) -> &'static str {
        "identifier-renamed"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn content_path(&self) -> Option<&str> {
        Some(&self.path)
    }

    fn line_position(&self) -> Option<(usize, usize)> {
        Some((self.old, self.new))
    }

    fn edits_lines(&self) -> bool {
        true
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

    fn rebuild_line(
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
            &self.path,
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

pub(crate) struct DirectoryAdded {
    pub(crate) path: String,
}

impl Record for DirectoryAdded {
    fn kind(&self) -> &'static str {
        "directory-added"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn scope(&self) -> &str {
        "."
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
    }

    fn apply(&self, source: &Path) -> Result<(), String> {
        crate::apply::add_directory(source, &self.path)
    }
}

pub(crate) struct DirectoryDeleted {
    pub(crate) path: String,
}

impl Record for DirectoryDeleted {
    fn kind(&self) -> &'static str {
        "directory-deleted"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn scope(&self) -> &str {
        "."
    }

    fn removed_directory(&self) -> Option<&str> {
        Some(&self.path)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_detail(document, "path", &self.path);
    }

    fn apply(&self, source: &Path) -> Result<(), String> {
        crate::apply::remove_directory(source, &self.path)
    }
}
