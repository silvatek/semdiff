/// A line comment that is present in the right-hand source and not the left.
pub(crate) struct AddedLineComment {
    pub scope: String,
    pub line: usize,
    pub text: String,
}

/// An inserted line that is a comment in this language.
///
/// The line was already found by comparing the file. A trailing note on a code line
/// is not a comment line, so only a line that the dialect reads as a comment counts.
/// The line number is where that line sits in the new file.
pub(crate) fn inserted_comment(
    scope: &str,
    line: usize,
    text: &str,
    is_line_comment: fn(&str) -> bool,
) -> Option<AddedLineComment> {
    if !(is_line_comment)(text) {
        return None;
    }
    Some(AddedLineComment {
        scope: scope.to_string(),
        line,
        text: text.to_string(),
    })
}
