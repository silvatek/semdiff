use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A symbol rename that covers every file in one scope.
///
/// The old name is gone from that scope, and the new name appears the same number of
/// times, so the files share one record.
pub(crate) struct SymbolRename {
    pub scope: String,
    pub from: String,
    pub to: String,
}

/// Symbol renames across the paired files.
///
/// Each file is read with its own dialect. A name that is still used in the scope is
/// left unrecorded, because that rename would have to be split into smaller scopes.
pub(crate) fn symbol_renames(
    sources: &crate::Sources,
    paths: &[String],
) -> Result<Vec<SymbolRename>, String> {
    let mut left_counts = BTreeMap::new();
    let mut right_counts = BTreeMap::new();
    for path in paths {
        let Some(dialect) = crate::dialect::for_path(path) else {
            continue;
        };
        let Some(identifiers) = dialect.identifiers else {
            continue;
        };
        let left_path = source_file(&sources.left, path);
        let right_path = source_file(&sources.right, path);
        let left = fs::read_to_string(&left_path)
            .map_err(|error| format!("cannot read {}: {error}", left_path.display()))?;
        let right = fs::read_to_string(&right_path)
            .map_err(|error| format!("cannot read {}: {error}", right_path.display()))?;
        for name in identifiers(&left) {
            *left_counts.entry(name).or_default() += 1;
        }
        for name in identifiers(&right) {
            *right_counts.entry(name).or_default() += 1;
        }
    }
    Ok(pair_renames(&left_counts, &right_counts)
        .into_iter()
        .map(|(from, to)| SymbolRename {
            scope: ".".to_string(),
            from,
            to,
        })
        .collect())
}

/// Renames one code identifier in every file of `scope`.
///
/// Each file is rewritten by its dialect, so comments, strings, and keywords stay as
/// that language writes them.
pub(crate) fn apply_rename(root: &Path, scope: &str, from: &str, to: &str) -> Result<(), String> {
    let base = if scope == "." {
        root.to_path_buf()
    } else {
        root.join(scope)
    };
    if base.is_file() {
        return rewrite_file(&base, from, to);
    }
    for relative in crate::collect_files(&base)? {
        rewrite_file(&base.join(relative), from, to)?;
    }
    Ok(())
}

fn pair_renames(
    left: &BTreeMap<String, usize>,
    right: &BTreeMap<String, usize>,
) -> Vec<(String, String)> {
    let mut removed: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    let mut added: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for (name, count) in left {
        if right.get(name).copied().unwrap_or(0) == 0 {
            removed.entry(*count).or_default().push(name);
        }
    }
    for (name, count) in right {
        if left.get(name).copied().unwrap_or(0) == 0 {
            added.entry(*count).or_default().push(name);
        }
    }
    let mut pairs = Vec::new();
    for (count, from_names) in &removed {
        let Some(to_names) = added.get(count) else {
            continue;
        };
        if from_names.len() == 1 && to_names.len() == 1 {
            pairs.push((from_names[0].to_string(), to_names[0].to_string()));
        }
    }
    pairs.sort();
    pairs
}

fn source_file(source: &str, relative: &str) -> PathBuf {
    crate::source_base(Path::new(source)).join(relative)
}

fn rewrite_file(path: &Path, from: &str, to: &str) -> Result<(), String> {
    let Some(dialect) = crate::dialect::for_path(&path.to_string_lossy()) else {
        return Ok(());
    };
    let Some(rename) = dialect.rename_identifier else {
        return Ok(());
    };
    let original = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let updated = rename(&original, from, to);
    if updated != original {
        fs::write(path, updated)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::pair_renames;

    #[test]
    fn pairs_one_renamed_symbol_when_the_count_matches() {
        let left = BTreeMap::from([("fibonacci".to_string(), 7), ("kept".to_string(), 2)]);
        let right = BTreeMap::from([("fib".to_string(), 7), ("kept".to_string(), 2)]);
        assert_eq!(
            pair_renames(&left, &right),
            vec![("fibonacci".to_string(), "fib".to_string())]
        );
    }

    #[test]
    fn skips_a_symbol_that_is_still_used() {
        let left = BTreeMap::from([("fibonacci".to_string(), 2)]);
        let right = BTreeMap::from([("fibonacci".to_string(), 1), ("fib".to_string(), 1)]);
        assert!(pair_renames(&left, &right).is_empty());
    }

    #[test]
    fn skips_ambiguous_renames() {
        let left = BTreeMap::from([("alpha".to_string(), 2), ("beta".to_string(), 2)]);
        let right = BTreeMap::from([("gamma".to_string(), 2), ("delta".to_string(), 2)]);
        assert!(pair_renames(&left, &right).is_empty());
    }
}
