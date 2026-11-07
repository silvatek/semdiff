//! Identifying and classifying changes at the directory level.

use std::collections::BTreeSet;

use crate::change::Change;

/// Directories added or removed between the two sources.
///
/// A directory that exists only on the right, and holds no files, was added.
/// A directory that exists only on the left was removed, including one that
/// becomes empty because its files moved or were deleted. Removals come first,
/// nested directories before their parents, so a directory is empty when it is
/// removed. Additions follow, parents before their children.
pub(crate) fn directory_changes(
    left_dirs: &[String],
    right_dirs: &[String],
    right_files: &[String],
) -> Vec<Box<dyn Change>> {
    let left_names: BTreeSet<_> = left_dirs.iter().collect();
    let right_names: BTreeSet<_> = right_dirs.iter().collect();
    let mut removed: Vec<_> = left_dirs
        .iter()
        .filter(|path| !right_names.contains(path))
        .cloned()
        .collect();
    removed.sort_by(|left, right| {
        directory_depth(right)
            .cmp(&directory_depth(left))
            .then(right.cmp(left))
    });
    let mut added: Vec<_> = right_dirs
        .iter()
        .filter(|path| !left_names.contains(path) && !contains_file(path, right_files))
        .cloned()
        .collect();
    added.sort_by(|left, right| {
        directory_depth(left)
            .cmp(&directory_depth(right))
            .then(left.cmp(right))
    });

    let mut changes: Vec<Box<dyn Change>> = Vec::new();
    for path in removed {
        changes.push(Box::new(crate::change::DirectoryDeleted {
            scope: crate::change::TREE_SCOPE.to_string(),
            path,
        }));
    }
    for path in added {
        changes.push(Box::new(crate::change::DirectoryAdded {
            scope: crate::change::TREE_SCOPE.to_string(),
            path,
        }));
    }
    changes
}

/// Puts a file that replaces a removed directory after that directory is removed.
///
/// The file cannot be written while its path is still a directory. Deleting the
/// files inside, then the directory, leaves the path free for the new file.
pub(crate) fn place_replacement_files(changes: Vec<Box<dyn Change>>) -> Vec<Box<dyn Change>> {
    let removed: BTreeSet<_> = changes
        .iter()
        .filter_map(|change| {
            change
                .removed_directory()
                .map(|dir| dir.directory().to_string())
        })
        .collect();
    let mut kept = Vec::new();
    let mut replacements = Vec::new();
    for change in changes {
        if change
            .added_file()
            .is_some_and(|file| removed.contains(file.path()))
        {
            replacements.push(change);
        } else {
            kept.push(change);
        }
    }
    kept.extend(replacements);
    kept
}

fn directory_depth(path: &str) -> usize {
    path.matches('/').count()
}

fn contains_file(directory: &str, files: &[String]) -> bool {
    let prefix = format!("{directory}/");
    files.iter().any(|path| path.starts_with(&prefix))
}
