mod apply;
mod args;
mod comment;
mod dialect;
mod rename;
mod semdif;
mod symbol;

pub(crate) use args::Sources;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use semdif::{Change, ChangeDetails, ChangeType};

/// Compares two sources, or applies a semdif file to one source.
///
/// Argument errors stop the command before any comparison or edit starts.
fn main() -> ExitCode {
    match args::parse_args() {
        args::Command::Compare(args) => {
            let differences = find_differences(&args.sources);
            semdif::write_output(&args.output, &args.sources, &differences)
        }
        args::Command::Apply { source, patch } => apply::apply(&source, &patch),
    }
}

struct Differences {
    paths: Vec<String>,
    changes: Vec<Change>,
}

/// The shared files and the ordered changes between the two sources.
///
/// File renames come first. Each paired file is then compared on its own: the lines
/// that differ, the spans that differ inside those lines, and what each span is.
/// Identifier renames that name the same thing are then collapsed into one wider scope.
fn find_differences(sources: &Sources) -> Differences {
    let paths = paired_files(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let mut changes = file_rename_changes(sources).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    let (content, paired) = changed_content(sources, &paths).unwrap_or_else(|message| {
        exit_with_message(message);
    });
    changes.extend(content);
    Differences {
        paths,
        changes: collapse_scopes(changes, &paired),
    }
}

/// File renames, before any file contents are compared.
///
/// A renamed file keeps its lines, so the line delta is zero.
fn file_rename_changes(sources: &Sources) -> Result<Vec<Change>, String> {
    Ok(rename::file_renames(sources)?
        .into_iter()
        .map(|(from, to)| Change::file_renamed(from, to))
        .collect())
}

/// One paired file's text, kept so a later collapse can see a use that was left as it was.
///
/// The path is the same on both sides. A renamed file is recorded before this step.
struct PairedText {
    path: String,
    left: String,
    right: String,
}

/// Content changes in the files that kept the same path.
///
/// A renamed file is already recorded, so the dialect is the one named by the extension.
/// Text that did not change is kept so a later collapse can see a use that was left as it was.
fn changed_content(
    sources: &Sources,
    paths: &[String],
) -> Result<(Vec<Change>, Vec<PairedText>), String> {
    let mut changes = Vec::new();
    let mut paired = Vec::new();
    for path in paths {
        let Some(dialect) = dialect_for(path) else {
            continue;
        };
        let left = read_text(&sources.left, path)?;
        let right = read_text(&sources.right, path)?;
        if left != right {
            changes.extend(changes_in_file(path, dialect, &left, &right));
        }
        paired.push(PairedText {
            path: path.clone(),
            left,
            right,
        });
    }
    Ok((changes, paired))
}

/// The dialect for a file that kept its path.
///
/// Both sides use that path, so the extension is enough to choose the dialect.
fn dialect_for(path: &str) -> Option<&'static crate::dialect::Dialect> {
    crate::dialect::for_path(path)
}

/// The changes found by walking one file's differing lines, then the spans in those lines.
///
/// An inserted line that is a comment is recorded on its own. Spans are named only once
/// an edited line has spans to identify. An identifier is added to a list, and once every
/// line has been read that list becomes a symbol rename when every use of the name changed.
fn changes_in_file(
    path: &str,
    dialect: &crate::dialect::Dialect,
    left: &str,
    right: &str,
) -> Vec<Change> {
    let mut changes = Vec::new();
    let lines = symbol::changed_lines(left, right);
    for line in &lines {
        let symbol::ChangedLine::Inserted { line, text } = line else {
            continue;
        };
        if let Some(added) = comment::inserted_comment(path, *line, text, dialect.is_line_comment) {
            changes.push(Change::line_comment_added(
                added.scope,
                added.line,
                added.text,
            ));
        }
    }
    changes.extend(symbol_renames(path, dialect, left, right, &lines));
    changes
}

/// Symbol renames in one paired file.
///
/// Spans are named only when an edited line has spans to identify. The file then
/// becomes a rename when every use of that name changed to the same new name.
fn symbol_renames(
    path: &str,
    dialect: &crate::dialect::Dialect,
    left: &str,
    right: &str,
    lines: &[symbol::ChangedLine],
) -> Vec<Change> {
    if !lines
        .iter()
        .any(|line| matches!(line, symbol::ChangedLine::Edited(_)))
    {
        return Vec::new();
    }
    let Some(file) = named_spans(path, dialect, left, right) else {
        return Vec::new();
    };
    let mut identifiers = Vec::new();
    for line in lines {
        let symbol::ChangedLine::Edited(line) = line else {
            continue;
        };
        for (before, after) in
            symbol::changed_spans(left, right, line, file.left_spans(), file.right_spans())
        {
            if before.kind == crate::dialect::SpanKind::Identifier
                && after.kind == crate::dialect::SpanKind::Identifier
                && before.text != after.text
            {
                identifiers.push((before.text.to_string(), after.text.to_string()));
            }
        }
    }
    symbol::renames_in_file(&file, &identifiers)
        .into_iter()
        .map(|rename| Change::symbol_renamed(rename.scope, rename.from, rename.to))
        .collect()
}

/// The paired file with its spans named, when this dialect can name them.
///
/// A dialect that cannot name spans yet has no symbol renames.
fn named_spans(
    path: &str,
    dialect: &crate::dialect::Dialect,
    left: &str,
    right: &str,
) -> Option<symbol::ComparedFile> {
    dialect
        .spans
        .map(|classify| symbol::compared_file(path, left.to_string(), right.to_string(), classify))
}

/// Joins changes that are the same rename and can share a wider scope.
///
/// The wider scope is kept only when it contains no use that was left unchanged.
/// File renames stay first, then symbol renames, then added comments in line order,
/// so line deltas can be followed from the start of the list.
fn collapse_scopes(changes: Vec<Change>, files: &[PairedText]) -> Vec<Change> {
    let mut renames = Vec::new();
    let mut symbols = Vec::new();
    let mut comments = Vec::new();
    for change in changes {
        match change.change_type {
            ChangeType::FileRenamed => renames.push(change),
            ChangeType::SymbolRenamed => symbols.push(change),
            ChangeType::LineCommentAdded => comments.push(change),
        }
    }
    renames.extend(collapse_symbol_renames(symbols, files));
    renames.extend(comments);
    renames
}

/// Symbol renames after file-sized changes that name the same identifier have been joined.
///
/// Each joined change keeps one scope, the smallest directory or file that contains
/// the renamed uses and no use that was left as it was.
fn collapse_symbol_renames(changes: Vec<Change>, files: &[PairedText]) -> Vec<Change> {
    let files = files_with_uses(files);
    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for change in changes {
        let ChangeDetails::SymbolRenamed { from, to } = &change.details else {
            continue;
        };
        groups
            .entry((from.clone(), to.clone()))
            .or_default()
            .push(change.scope.clone());
    }
    let mut collapsed = Vec::new();
    for ((from, to), scopes) in groups {
        let mut clusters = clusters(&scopes, &from, &files);
        clusters.sort_by(|left, right| {
            symbol::tightest_scope(&str_refs(left)).cmp(&symbol::tightest_scope(&str_refs(right)))
        });
        for cluster in clusters {
            collapsed.push(Change::symbol_renamed(
                symbol::tightest_scope(&str_refs(&cluster)),
                from.clone(),
                to.clone(),
            ));
        }
    }
    collapsed
}

/// Paired files in which a remaining use of a renamed name can be seen.
///
/// A wider scope is kept only when none of them still has the old name.
fn files_with_uses(files: &[PairedText]) -> Vec<symbol::ComparedFile> {
    files
        .iter()
        .filter_map(|file| {
            named_spans(
                &file.path,
                dialect_for(&file.path)?,
                &file.left,
                &file.right,
            )
        })
        .collect()
}

/// Groups of files that can share one scope for a rename of `from`.
///
/// Two files are grouped when the directory that contains both does not also contain
/// a use that still has the old name.
fn clusters(scopes: &[String], from: &str, files: &[symbol::ComparedFile]) -> Vec<Vec<String>> {
    let mut clusters: Vec<Vec<String>> = scopes.iter().cloned().map(|scope| vec![scope]).collect();
    let mut merged = true;
    while merged {
        merged = false;
        let mut index = 0;
        while index < clusters.len() {
            let mut other = index + 1;
            while other < clusters.len() {
                let mut paths = clusters[index].clone();
                paths.extend(clusters[other].iter().cloned());
                let scope = symbol::tightest_scope(&str_refs(&paths));
                if scope_covers_only(&scope, &paths, from, files) {
                    clusters[index] = paths;
                    clusters.remove(other);
                    merged = true;
                } else {
                    other += 1;
                }
            }
            index += 1;
        }
    }
    clusters
}

/// Whether every use of `from` inside `scope` belongs to `paths`.
///
/// A directory that still contains the old name cannot be the scope of the rename.
fn scope_covers_only(
    scope: &str,
    paths: &[String],
    from: &str,
    files: &[symbol::ComparedFile],
) -> bool {
    files.iter().all(|file| {
        !file.has_use(from)
            || paths.iter().any(|path| path == file.path())
            || !scope_contains(scope, file.path())
    })
}

/// Whether `path` is the scope itself or a file inside it.
fn scope_contains(scope: &str, path: &str) -> bool {
    scope == "." || scope == path || path.starts_with(&format!("{scope}/"))
}

/// Borrowed paths, so a scope can be chosen without copying them again.
fn str_refs(paths: &[String]) -> Vec<&str> {
    paths.iter().map(String::as_str).collect()
}

/// The text of one paired file.
///
/// The comparison classifies this text, so the file is not read again for each change type.
fn read_text(source: &str, relative: &str) -> Result<String, String> {
    let path = source_base(Path::new(source)).join(relative);
    fs::read_to_string(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// Stops the command after a comparison failure.
///
/// The reason is shown on stderr and the process exits with code 1.
fn exit_with_message(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

/// The directory from which a source's relative paths are read.
///
/// A single file and a directory tree can then be read in the same way.
pub(crate) fn source_base(path: &Path) -> &Path {
    if path.is_file() {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    } else {
        path
    }
}

/// The files that can be compared in place because they exist in both sources.
///
/// The paths are in a stable order so repeated runs list the same files.
fn paired_files(sources: &Sources) -> Result<Vec<String>, String> {
    let right_files: BTreeSet<_> = collect_files(Path::new(&sources.right))?
        .into_iter()
        .collect();
    let mut shared: Vec<_> = collect_files(Path::new(&sources.left))?
        .into_iter()
        .filter(|path| right_files.contains(path))
        .collect();
    shared.sort();
    shared.dedup();
    Ok(shared)
}

/// The files that belong to a source.
///
/// A directory contributes its project files, and a file contributes itself. Generated
/// build output is omitted so it is not treated as project content.
pub(crate) fn collect_files(root: &Path) -> Result<Vec<String>, String> {
    if root.is_file() {
        let name = root
            .file_name()
            .ok_or_else(|| format!("cannot name {}", root.display()))?;
        return Ok(vec![name.to_string_lossy().into_owned()]);
    }
    let mut files = Vec::new();
    visit_dir(root, root, &mut files)?;
    Ok(files)
}

/// The files contained in a directory and its subdirectories.
///
/// Nested source files belong in the comparison, and generated build output does not.
fn visit_dir(root: &Path, dir: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            visit_dir(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("cannot relativize {}", path.display()))?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}
