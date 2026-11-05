//! One change in a semdif file.
//!
//! Each kind of change is its own type and implements [`GenericChange`]. That
//! trait is only what every kind has: its semdif type, how widely it applies,
//! the details it writes, and how it is replayed on its own. A kind that also
//! edits lines, replaces a file, or removes a directory implements that
//! narrower trait and reveals it from [`GenericChange`].

mod directory;
mod file;
mod line;

pub(crate) use directory::{DirectoryAdded, DirectoryDeleted};
pub(crate) use file::{FileAdded, FileChanged, FileDeleted, FileRenamed};
pub(crate) use line::{merge_adjacent, IdentifierRenamed, LineAdded, LineModified, LineRemoved};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Scope written for a change that is not the contents of one file.
///
/// A rename, an added or deleted file, and an added or deleted directory use
/// this scope. A change to the lines or bytes of one file is scoped to that
/// file. A later change that spans several files, such as renaming files by a
/// regular expression, stores the scope that says how widely it applies.
pub(crate) const TREE_SCOPE: &str = ".";

/// A change a semdif file can record.
///
/// The value is a trait object so the list can hold every kind. The concrete
/// type supplies the fields and the operations that differ.
pub(crate) type Change = Box<dyn GenericChange>;

/// What every change has.
///
/// A kind reveals the narrower traits it implements. The defaults say that
/// this kind does not.
pub(crate) trait GenericChange {
    /// The semdif type name, such as `line-removed`.
    fn kind(&self) -> &'static str;

    /// How widely this change applies.
    ///
    /// The value is stored on the change. A change to one file returns that
    /// file. A change that spans several files returns the scope that limits
    /// where it is applied, such as the directory a regular-expression rename
    /// covers. A change of the tree itself returns [`TREE_SCOPE`].
    fn scope(&self) -> &str;

    /// The pair used to order changes before they are written.
    ///
    /// A content edit is ordered by its file. A tree change overrides this
    /// when it is ordered by a path in its details.
    fn sort_key(&self) -> (&str, &str) {
        (self.scope(), "")
    }

    /// Writes the detail lines, under a `details` key the driver already opened.
    fn write_details(&self, document: &mut String);

    /// Replays this change on its own.
    ///
    /// A content edit is replayed with the other edits of its file, so its
    /// implementation reports that instead of changing anything.
    fn apply(&self, source: &Path) -> Result<(), String>;

    /// This change edits lines of its file, when it does.
    fn line_edit(&self) -> Option<&dyn LineEdit> {
        None
    }

    /// The mutable line edit, when merging needs to append to it.
    fn line_edit_mut(&mut self) -> Option<&mut dyn LineEdit> {
        None
    }

    /// This change is applied with the other edits of its file, when it is.
    fn content(&self) -> Option<&dyn Content> {
        None
    }

    /// The old and new hashes of a changed file, when this change carries them.
    fn digest(&self) -> Option<&dyn Digest> {
        None
    }

    /// The bytes that replace a whole file, when this change carries them.
    fn replacement(&self) -> Option<&dyn Replacement> {
        None
    }

    /// The directory this change removes, when it removes one.
    fn removed_directory(&self) -> Option<&dyn RemovedDirectory> {
        None
    }

    /// The file this change adds, when adding that file must wait for a
    /// directory at the same path to be removed.
    fn added_file(&self) -> Option<&dyn AddedFile> {
        None
    }
}

/// A change applied with the other edits of one file.
///
/// Apply groups these by [`Content::file`], checks the old hash, builds the
/// new bytes, checks the new hash, and writes the file once. For a change to
/// one file, that file is the change's scope.
pub(crate) trait Content {
    /// The file those edits share.
    fn file(&self) -> &str;
}

/// The old and new SHA-256 hex hashes of one changed file.
pub(crate) trait Digest {
    fn old_hash(&self) -> &str;
    fn new_hash(&self) -> &str;
}

/// Bytes that replace a whole file.
pub(crate) trait Replacement {
    fn bytes(&self) -> &[u8];
}

/// An edit of one or more lines.
///
/// `extend` stays separate unless this kind joins an adjacent run of the same
/// kind. An identifier rename does not.
pub(crate) trait LineEdit {
    /// `old`, then `new`.
    fn position(&self) -> (usize, usize);

    /// Adds this edit to the file's [`LinePlan`].
    ///
    /// Apply calls this for every line edit of one file, then
    /// [`LinePlan::rebuild`] turns that plan into the file's new lines.
    fn rebuild(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String>;

    /// Joins `next` onto this edit when the two are one adjacent run.
    fn extend(&mut self, _next: &dyn GenericChange) -> bool {
        false
    }

    /// The first old line and the first new line, when this edit can merge.
    fn span(&self) -> Option<LineSpan> {
        None
    }

    /// The line text a following edit would append, when this edit can merge.
    fn text(&self) -> Option<&str> {
        None
    }
}

/// A directory removed from the tree.
pub(crate) trait RemovedDirectory {
    fn directory(&self) -> &str;
}

/// A file added to the tree.
pub(crate) trait AddedFile {
    fn path(&self) -> &str;
}

/// The first old line and the first new line of a line edit.
///
/// The length on each side is known by the change being extended. A removal
/// has a new length of zero, so `new` is the line that follows it. An addition
/// has an old length of zero, so `old` is the line it is inserted before.
#[derive(Clone, Copy)]
pub(crate) struct LineSpan {
    pub(super) old: usize,
    pub(super) new: usize,
}

/// Line edits gathered for one file during apply.
///
/// [`crate::apply`] is the only user. When a file's changes include line edits
/// and do not replace the whole file, apply creates one plan, asks each line
/// edit to add itself, and then rebuilds the file's lines from it. A rename,
/// an added or deleted file, a directory change, a hash with no line edits, or
/// a whole-file replacement never builds one. See [`LinePlan::rebuild`].
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

    pub(super) fn replace(
        &mut self,
        file: &Path,
        line_no: usize,
        text: String,
    ) -> Result<(), String> {
        if self.modified.insert(line_no, text).is_some() {
            return Err(format!(
                "line {line_no} is modified more than once in {}",
                file.display()
            ));
        }
        Ok(())
    }

    pub(super) fn remove_range(
        &mut self,
        file: &Path,
        old: usize,
        old_end: usize,
    ) -> Result<(), String> {
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

    pub(super) fn insert(
        &mut self,
        file: &Path,
        old: usize,
        new: usize,
        text: &str,
    ) -> Result<(), String> {
        for (offset, line) in text.split('\n').enumerate() {
            let line_no = new
                .checked_add(offset)
                .ok_or_else(|| format!("cannot insert before line {old} in {}", file.display()))?;
            self.added.push((old, line_no, line.to_string()));
        }
        Ok(())
    }

    /// Builds the new lines of one file from the edits gathered here.
    ///
    /// Apply calls this once, after the file's old hash has matched, for a
    /// text file whose changes are line edits. The lines are hashed and
    /// written only when that new hash matches the semdif file. A mismatch
    /// leaves the file on disk unchanged, and this plan is dropped with it.
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

pub(super) fn applied_with_its_file(kind: &str) -> Result<(), String> {
    Err(format!("{kind} is applied with its file"))
}
