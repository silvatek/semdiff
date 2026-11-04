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
/// A name that is still used in the scope is left unrecorded, because that rename would
/// have to be split into smaller scopes.
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
/// Comments and strings keep the old text, because they are not uses of the symbol.
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

/// Code identifiers in a Rust source.
pub(crate) fn rust_identifiers(source: &str) -> Vec<String> {
    scan_rust(source, None).0
}

/// Rust source with one code identifier renamed.
pub(crate) fn rust_rename_identifier(source: &str, from: &str, to: &str) -> String {
    scan_rust(source, Some((from, to))).1
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

fn scan_rust(source: &str, rename: Option<(&str, &str)>) -> (Vec<String>, String) {
    let mut index = 0;
    let mut identifiers = Vec::new();
    let mut output = String::new();
    while index < source.len() {
        if source[index..].starts_with("//") {
            copy_line_comment(source, &mut index, &mut output);
            continue;
        }
        if source[index..].starts_with("/*") {
            copy_block_comment(source, &mut index, &mut output);
            continue;
        }
        if peek(source, index) == Some('\'') {
            copy_char_or_lifetime(source, &mut index, &mut output);
            continue;
        }
        if peek(source, index) == Some('"') {
            copy_cooked_string(source, &mut index, &mut output);
            continue;
        }
        if matches!(peek(source, index), Some(character) if is_ident_start(character)) {
            let start = index;
            let ident = read_ident(source, &mut index);
            if take_rust_literal(source, &mut index, &mut output, &ident, start) {
                continue;
            }
            record_ident(&mut identifiers, &mut output, &ident, rename);
            continue;
        }
        copy_one(source, &mut index, &mut output);
    }
    (identifiers, output)
}

fn record_ident(
    identifiers: &mut Vec<String>,
    output: &mut String,
    ident: &str,
    rename: Option<(&str, &str)>,
) {
    identifiers.push(ident.to_string());
    if let Some((from, to)) = rename {
        if ident == from {
            output.push_str(to);
            return;
        }
    }
    output.push_str(ident);
}

fn take_rust_literal(
    source: &str,
    index: &mut usize,
    output: &mut String,
    ident: &str,
    ident_start: usize,
) -> bool {
    let after_ident = *index;
    if ident == "r" && matches!(peek(source, *index), Some('"' | '#')) {
        let mut cursor = *index;
        let hashes = count_hashes(source, &mut cursor);
        if peek(source, cursor) == Some('"') {
            output.push_str(&source[ident_start..cursor]);
            *index = cursor;
            copy_raw_string(source, index, output, hashes);
            return true;
        }
        return false;
    }
    if matches!(ident, "br" | "cr") && matches!(peek(source, *index), Some('"' | '#')) {
        let mut cursor = *index;
        let hashes = count_hashes(source, &mut cursor);
        if peek(source, cursor) == Some('"') {
            output.push_str(&source[ident_start..cursor]);
            *index = cursor;
            copy_raw_string(source, index, output, hashes);
            return true;
        }
        return false;
    }
    if matches!(ident, "b" | "c") {
        if peek(source, *index) == Some('"') {
            output.push_str(&source[ident_start..after_ident]);
            copy_cooked_string(source, index, output);
            return true;
        }
        if peek(source, *index) == Some('\'') {
            output.push_str(&source[ident_start..after_ident]);
            copy_char_or_lifetime(source, index, output);
            return true;
        }
        if source[*index..].starts_with("r\"") || source[*index..].starts_with("r#") {
            let mut cursor = *index + 1;
            let hashes = count_hashes(source, &mut cursor);
            if peek(source, cursor) == Some('"') {
                output.push_str(&source[ident_start..cursor]);
                *index = cursor;
                copy_raw_string(source, index, output, hashes);
                return true;
            }
        }
    }
    false
}

fn copy_line_comment(source: &str, index: &mut usize, output: &mut String) {
    while *index < source.len() {
        let character = peek(source, *index);
        copy_one(source, index, output);
        if character == Some('\n') {
            break;
        }
    }
}

fn copy_block_comment(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
    *index += 2;
    let mut depth = 1;
    while *index < source.len() && depth > 0 {
        if source[*index..].starts_with("/*") {
            depth += 1;
            *index += 2;
        } else if source[*index..].starts_with("*/") {
            depth -= 1;
            *index += 2;
        } else {
            let _ = eat(source, index);
        }
    }
    output.push_str(&source[start..*index]);
}

fn copy_cooked_string(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
    let _ = eat(source, index);
    while *index < source.len() {
        match peek(source, *index) {
            Some('\\') => {
                let _ = eat(source, index);
                let _ = eat(source, index);
            }
            Some('"') => {
                let _ = eat(source, index);
                break;
            }
            Some(_) => {
                let _ = eat(source, index);
            }
            None => break,
        }
    }
    output.push_str(&source[start..*index]);
}

fn copy_raw_string(source: &str, index: &mut usize, output: &mut String, hashes: usize) {
    let start = *index;
    let _ = eat(source, index);
    let marker = "#".repeat(hashes);
    while *index < source.len() {
        if peek(source, *index) == Some('"') && source[*index + 1..].starts_with(&marker) {
            *index += 1 + hashes;
            break;
        }
        let _ = eat(source, index);
    }
    output.push_str(&source[start..*index]);
}

fn copy_char_or_lifetime(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
    let _ = eat(source, index);
    if peek(source, *index) == Some('\\') {
        let _ = eat(source, index);
        if peek(source, *index) == Some('u') {
            let _ = eat(source, index);
            if peek(source, *index) == Some('{') {
                while *index < source.len() && peek(source, *index) != Some('}') {
                    let _ = eat(source, index);
                }
                if peek(source, *index) == Some('}') {
                    let _ = eat(source, index);
                }
            }
        } else {
            let _ = eat(source, index);
        }
        if peek(source, *index) == Some('\'') {
            let _ = eat(source, index);
        }
        output.push_str(&source[start..*index]);
        return;
    }
    if matches!(peek(source, *index), Some(character) if is_ident_start(character)) {
        let name = read_ident(source, index);
        if name.chars().count() == 1 && peek(source, *index) == Some('\'') {
            let _ = eat(source, index);
        }
    }
    output.push_str(&source[start..*index]);
}

fn count_hashes(source: &str, index: &mut usize) -> usize {
    let mut count = 0;
    while peek(source, *index) == Some('#') {
        count += 1;
        *index += 1;
    }
    count
}

fn read_ident(source: &str, index: &mut usize) -> String {
    let start = *index;
    if matches!(peek(source, *index), Some(character) if is_ident_start(character)) {
        let _ = eat(source, index);
        while matches!(peek(source, *index), Some(character) if is_ident_continue(character)) {
            let _ = eat(source, index);
        }
    }
    source[start..*index].to_string()
}

fn copy_one(source: &str, index: &mut usize, output: &mut String) {
    let start = *index;
    let _ = eat(source, index);
    output.push_str(&source[start..*index]);
}

fn eat(source: &str, index: &mut usize) -> Option<char> {
    let character = peek(source, *index)?;
    *index += character.len_utf8();
    Some(character)
}

fn peek(source: &str, index: usize) -> Option<char> {
    source[index..].chars().next()
}

fn is_ident_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_ident_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{pair_renames, rust_identifiers, rust_rename_identifier};

    #[test]
    fn rust_identifiers_skip_comments_and_strings() {
        let source = concat!(
            "fn fibonacci() {\n",
            "    /* fibonacci /* still */ fibonacci */\n",
            "    \"fibonacci\"\n",
            "    r#\"fibonacci\"#\n",
            "    // fibonacci\n",
            "    fibonacci()\n",
            "}\n",
        );
        let names: Vec<_> = rust_identifiers(source)
            .into_iter()
            .filter(|name| name == "fibonacci")
            .collect();
        assert_eq!(names, ["fibonacci", "fibonacci"]);
    }

    #[test]
    fn rust_rename_keeps_comments_and_strings() {
        let source = concat!(
            "fn fibonacci() {\n",
            "    // fibonacci\n",
            "    let s = \"fibonacci\";\n",
            "    println!(\"{n}: {}\", fibonacci(n));\n",
            "}\n",
        );
        let renamed = rust_rename_identifier(source, "fibonacci", "fib");
        assert_eq!(
            renamed,
            concat!(
                "fn fib() {\n",
                "    // fibonacci\n",
                "    let s = \"fibonacci\";\n",
                "    println!(\"{n}: {}\", fib(n));\n",
                "}\n",
            )
        );
    }

    #[test]
    fn pairs_one_renamed_symbol_when_the_count_matches() {
        let left = BTreeMap::from([("fibonacci".to_string(), 7), ("fn".to_string(), 2)]);
        let right = BTreeMap::from([("fib".to_string(), 7), ("fn".to_string(), 2)]);
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
