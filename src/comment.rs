use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// A line comment that is present in the right-hand source and not the left.
pub(crate) struct AddedLineComment {
    pub scope: String,
    pub line: usize,
    pub text: String,
}

/// Rust line comments added in paired Rust files.
///
/// A comment line can appear without the file itself being new, so those lines are
/// reported on their own. A line counts as added only when the right-hand file has
/// more copies of that comment than the left.
pub(crate) fn added_line_comments(
    sources: &crate::Sources,
    paths: &[String],
) -> Result<Vec<AddedLineComment>, String> {
    let mut added = Vec::new();
    for path in paths.iter().filter(|path| path.ends_with(".rs")) {
        let left_lines = read_lines(&source_file(&sources.left, path))?;
        let right_lines = read_lines(&source_file(&sources.right, path))?;
        added.extend(added_comments_in_file(path, &left_lines, &right_lines));
    }
    Ok(added)
}

/// Comments added in one paired file.
///
/// The reported line is where the comment appears in the right-hand file, which is the
/// file a reader now sees.
fn added_comments_in_file(
    scope: &str,
    left_lines: &[String],
    right_lines: &[String],
) -> Vec<AddedLineComment> {
    let mut remaining = HashMap::<&str, usize>::new();
    for line in left_lines.iter().filter(|line| is_line_comment(line)) {
        *remaining.entry(line).or_default() += 1;
    }

    let mut added = Vec::new();
    for (index, line) in right_lines.iter().enumerate() {
        if !is_line_comment(line) {
            continue;
        }
        let count = remaining.get(line.as_str()).copied().unwrap_or(0);
        if count == 0 {
            added.push(AddedLineComment {
                scope: scope.to_string(),
                line: index + 1,
                text: line.clone(),
            });
        } else {
            remaining.insert(line, count - 1);
        }
    }
    added
}

/// Whether a line is a Rust line comment.
///
/// Ordinary `//` comments and the `///` and `//!` doc forms are all line comments, so an
/// added doc comment is still reported. Other edits in a paired file are outside this check.
fn is_line_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// The text lines of one source file.
///
/// Added comments are reported with their line number in the right-hand file.
fn read_lines(path: &Path) -> Result<Vec<String>, String> {
    let file =
        File::open(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    BufReader::new(file)
        .lines()
        .map(|line| line.map_err(|error| format!("cannot read {}: {error}", path.display())))
        .collect()
}

/// The filesystem path of a file inside a source.
///
/// Paired files are named relative to the source, and the comment check needs the real path.
fn source_file(source: &str, relative: &str) -> std::path::PathBuf {
    crate::source_base(Path::new(source)).join(relative)
}
