//! One change in a semdif file.
//!
//! Each kind of change is its own type and implements [`Change`]. That trait is
//! what every kind has: its semdif type, how widely it applies, the details it
//! writes, and how it is replayed on its own. A list holds `Box<dyn Change>` so
//! every kind can sit together. A kind that also edits lines, replaces a file,
//! or removes a directory implements that narrower trait and reveals it from
//! [`Change`].
//!
//! Directory changes, file changes, line changes, and span changes live in
//! their own modules. A span is a portion of one line. Its scope is `path:line`
//! or `path:start-end`. A rename that covers several files names the directory
//! instead.

mod directory;
mod file;
mod line;
mod span;

pub(crate) use directory::{DirectoryAdded, DirectoryDeleted};
pub(crate) use file::{FileAdded, FileChanged, FileDeleted, FileRenamed};
pub(crate) use line::{
    merge_adjacent, promote_named_blocks, LineAdded, LineModified, LineRemoved, NamedBlockAdded,
    NamedBlockRemoved,
};
pub(crate) use span::{
    merge_identifier_files, merge_identifier_spans, parse_rename_scope, IdentifierRenamed,
    RenameScope, SourceText, SpanScope,
};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::filesystem::FileSystem;

/// Scope written for a change that is not the contents of one file.
///
/// A rename, an added or deleted file, and an added or deleted directory use
/// this scope. A change to the lines or bytes of one file is scoped to that
/// file. A span change is scoped to one old line of that file, written
/// `path:line`, or to a run of old lines, written `path:start-end`. An
/// identifier rename that covers several files names the directory that
/// contains them, or this scope when that directory is the whole tree.
pub(crate) const TREE_SCOPE: &str = ".";

/// What every change has.
///
/// A list of changes holds `Box<dyn Change>`, so a rename, a line edit, and a
/// directory deletion can sit together. A kind reveals the narrower traits it
/// implements. The defaults say that this kind does not.
pub(crate) trait Change {
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
    fn apply(&self, fs: &dyn FileSystem, source: &Path) -> Result<(), String>;

    /// Confirms a recorded hash before [`Change::apply`].
    ///
    /// An added, deleted, or renamed file records the SHA-256 of its bytes.
    /// A mismatch is an error, so apply leaves the tree unchanged. A change
    /// with no hash succeeds. `--ignore-hash` skips this check and applies
    /// the change anyway.
    fn check_recorded_hash(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        Ok(())
    }

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

    /// This change renames an identifier, when it does.
    fn identifier_rename(&self) -> Option<IdentifierRename<'_>> {
        None
    }

    /// This change edits `path` when its file batch is applied.
    ///
    /// A content change edits the file it names. A directory-scoped rename
    /// edits every file under that directory.
    fn edits_file(&self, path: &str) -> bool {
        self.content().is_some_and(|item| item.file() == path)
    }

    /// The directory a span rename covers, when the scope names no lines.
    fn directory_scope(&self) -> Option<&str> {
        None
    }
}

/// The names and old lines of one identifier rename.
///
/// `file` is the file the rename already covers. `start` and `end` are the old
/// lines in that file. A rename of one line has the same start and end.
pub(crate) struct IdentifierRename<'a> {
    pub(crate) file: &'a str,
    pub(crate) oldname: &'a str,
    pub(crate) newname: &'a str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// A change applied with the other edits of one file.
///
/// Apply groups these by [`Content::file`], checks the old hash, builds the
/// new bytes, checks the new hash, and writes the file once. For a line
/// change, that file is the change's scope. For a span change, the scope is
/// `path:line` or `path:start-end` and the file is the path. A rename whose
/// scope is a directory joins the batch of each file it covers.
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
/// kind. A span change does not.
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
    fn extend(&mut self, _next: &dyn Change) -> bool {
        false
    }

    /// The first old line and the first new line, when this edit can be joined
    /// to the one before it.
    fn start(&self) -> Option<LineStart> {
        None
    }

    /// The line text a following edit would append, when this edit can merge.
    fn text(&self) -> Option<&str> {
        None
    }

    /// Appends one following line onto this edit.
    ///
    /// The text of a joined run is stored as one string, with a newline between
    /// lines. A kind that does not join, such as a span change, leaves its
    /// text unchanged and returns false.
    fn append_text(&mut self, _text: &str) -> bool {
        false
    }

    /// Whether this edit is a full-line comment.
    ///
    /// The semdif writes `isComment: true` and leaves the attribute out when
    /// this is false. A span change is not a comment change. Adjacent line
    /// edits join only when this flag matches.
    fn is_comment(&self) -> bool {
        false
    }

    /// Whether this edit is insignificant whitespace.
    ///
    /// The semdif writes `isWhitespace: true` and leaves the attribute out when
    /// this is false. A span change is not a whitespace change. Apply replays
    /// the line text and does not read this flag. A line may also be a comment.
    /// Adjacent line edits join only when this flag matches.
    fn is_whitespace(&self) -> bool {
        false
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

/// The first old line and the first new line of a line edit that can be joined
/// to the one before it.
///
/// The length on each side is known by the change being extended. A removal
/// has a new length of zero, so `new` is the line that follows it. An addition
/// has an old length of zero, so `old` is the line it is inserted before.
#[derive(Clone, Copy)]
pub(crate) struct LineStart {
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
