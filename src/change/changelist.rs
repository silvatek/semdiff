//! The list of changes from one comparison, or parsed from one semdif.

use std::collections::BTreeSet;

use super::{Change, FileMoved};

/// The changes gathered while one comparison runs, or parsed from a semdif.
///
/// The changes stay inside the list. A pass walks them in order, or reads one
/// by its index when it will move or replace that change. Adding, sorting,
/// moving, replacing, merging, or removing a change goes through this list.
/// A pass also asks the list for a moved file, the new paths those moves add,
/// or the paths of the old text.
pub(crate) struct ChangeList {
    changes: Vec<Box<dyn Change>>,
}

impl ChangeList {
    /// An empty list.
    pub(crate) fn new() -> Self {
        Self::of(Vec::new())
    }

    /// A list holding these changes.
    pub(crate) fn of(changes: Vec<Box<dyn Change>>) -> Self {
        Self { changes }
    }

    /// Appends the changes from a later stage of the comparison.
    pub(crate) fn append(&mut self, more: ChangeList) {
        self.changes.extend(more.changes);
    }

    /// Adds one change at the end of the list.
    ///
    /// The change is boxed here so a rename, a line edit, and a directory
    /// deletion can sit together.
    pub(crate) fn add(&mut self, change: impl Change + 'static) {
        self.changes.push(Box::new(change));
    }

    /// Orders the changes by the pair each change uses to sort.
    pub(crate) fn sort(&mut self) {
        self.changes
            .sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    }

    /// Orders the changes by old line, then new line.
    ///
    /// A change with no line edit sorts as line 0. A directory-scoped rename
    /// reports that same pair.
    pub(crate) fn sort_by_line(&mut self) {
        self.changes.sort_by_key(|change| {
            change
                .line_edit()
                .map(|line| line.position())
                .unwrap_or((0, 0))
        });
    }

    /// How many changes this list holds.
    pub(crate) fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether this list holds no changes.
    pub(crate) fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Lends the change at `index` and the change after it.
    ///
    /// `decide` can ask the first change whether it absorbs the second. The
    /// borrows end when `decide` returns, so the caller can then remove the
    /// second change. There is no following change when `index` is the last.
    pub(crate) fn with_next<R>(
        &mut self,
        index: usize,
        decide: impl FnOnce(&mut dyn Change, &dyn Change) -> R,
    ) -> Option<R> {
        if index + 1 >= self.changes.len() {
            return None;
        }
        let (earlier, later) = self.changes.split_at_mut(index + 1);
        Some(decide(earlier[index].as_mut(), later[0].as_ref()))
    }

    /// Moves the change at `from` so it sits at `to`.
    ///
    /// That change keeps its contents. The changes between the two indexes
    /// shift to close the gap.
    pub(crate) fn move_change(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        if from < to {
            self.changes[from..=to].rotate_left(1);
        } else {
            self.changes[to..=from].rotate_right(1);
        }
    }

    /// Puts `change` at `index` in place of the change already there.
    pub(crate) fn replace(&mut self, index: usize, change: Box<dyn Change>) {
        self.changes[index] = change;
    }

    /// Removes the change at `index` and returns it.
    pub(crate) fn remove(&mut self, index: usize) -> Box<dyn Change> {
        self.changes.remove(index)
    }

    /// Puts `change` at `index` and removes every index in `drop`.
    ///
    /// `change` is the merged change. `drop` lists the other changes folded
    /// into it. Each of those indexes is later in the list than `index`.
    pub(crate) fn merge(&mut self, index: usize, drop: &[usize], change: Box<dyn Change>) {
        self.replace(index, change);
        let mut later = drop.to_vec();
        later.sort_unstable();
        for position in later.into_iter().rev() {
            self.remove(position);
        }
    }

    /// The file-moved change whose new path is `path`.
    ///
    /// A comparison pass uses this when it has the file's new scope and needs
    /// the change that still names the old path.
    pub(crate) fn file_move_for(&self, path: &str) -> Option<&FileMoved> {
        for change in self {
            if let Some(moved) = change.file_move() {
                if moved.to == path {
                    return Some(moved);
                }
            }
        }
        None
    }

    /// The new path of each moved file.
    ///
    /// A move is compared with the files that share a path. The path in that
    /// list is the new path, and the changes to its contents are scoped to it.
    pub(crate) fn moved_destinations(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for change in self {
            if let Some(moved) = change.file_move() {
                paths.push(moved.to.clone());
            }
        }
        paths
    }

    /// The paths whose old text a later pass looks up.
    ///
    /// Each left-hand file is stored under its own path. A moved file is
    /// stored under its new path. The bytes are still read from the old path.
    pub(crate) fn old_text_paths(&self, left_files: &[String]) -> Vec<String> {
        let mut moved_from = BTreeSet::new();
        for change in self {
            if let Some(moved) = change.file_move() {
                moved_from.insert(moved.from.as_str());
            }
        }
        let mut paths = Vec::new();
        for path in left_files {
            if !moved_from.contains(path.as_str()) {
                paths.push(path.clone());
            }
        }
        paths.extend(self.moved_destinations());
        paths.sort();
        paths
    }
}

impl std::ops::Index<usize> for ChangeList {
    type Output = dyn Change;

    fn index(&self, index: usize) -> &Self::Output {
        self.changes[index].as_ref()
    }
}

/// Walks a change list from first to last.
pub(crate) struct ChangeListIter<'a> {
    inner: std::slice::Iter<'a, Box<dyn Change>>,
}

impl<'a> Iterator for ChangeListIter<'a> {
    type Item = &'a dyn Change;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|change| change.as_ref())
    }
}

impl<'a> IntoIterator for &'a ChangeList {
    type Item = &'a dyn Change;
    type IntoIter = ChangeListIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ChangeListIter {
            inner: self.changes.iter(),
        }
    }
}
