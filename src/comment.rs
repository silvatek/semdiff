use std::collections::HashMap;

/// A line comment that is present in the right-hand source and not the left.
pub(crate) struct AddedLineComment {
    pub scope: String,
    pub line: usize,
    pub text: String,
}

/// Line comments added in one paired file that has already been read.
///
/// A comment line can appear without the file itself being new, so those lines are
/// reported on their own. A line counts as added only when the right-hand file has
/// more copies of that comment than the left.
pub(crate) fn added_line_comments(
    path: &str,
    dialect: &crate::dialect::Dialect,
    left: &str,
    right: &str,
) -> Vec<AddedLineComment> {
    added_comments_in_file(path, dialect, &text_lines(left), &text_lines(right))
}

/// Comments added in one paired file.
///
/// The reported line is where the comment appears in the right-hand file, which is the
/// file a reader now sees.
fn added_comments_in_file(
    scope: &str,
    dialect: &crate::dialect::Dialect,
    left_lines: &[&str],
    right_lines: &[&str],
) -> Vec<AddedLineComment> {
    let mut remaining = HashMap::<&str, usize>::new();
    for line in left_lines
        .iter()
        .copied()
        .filter(|line| (dialect.is_line_comment)(line))
    {
        *remaining.entry(line).or_default() += 1;
    }

    let mut added = Vec::new();
    for (index, line) in right_lines.iter().copied().enumerate() {
        if !(dialect.is_line_comment)(line) {
            continue;
        }
        let count = remaining.get(line).copied().unwrap_or(0);
        if count == 0 {
            added.push(AddedLineComment {
                scope: scope.to_string(),
                line: index + 1,
                text: line.to_string(),
            });
        } else {
            remaining.insert(line, count - 1);
        }
    }
    added
}

/// The lines of a file that has already been read.
///
/// An added comment is reported with its line number in the right-hand file.
fn text_lines(source: &str) -> Vec<&str> {
    source.lines().collect()
}
