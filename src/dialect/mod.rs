mod rust;

/// A language semdiff knows how to read.
///
/// Extensions select its files. The line-comment test and the identifier readers select
/// which lines and names that language contributes, so another language can be added
/// without changing the comparison.
pub(crate) struct Dialect {
    /// Extensions that identify this dialect, including the leading dot.
    pub extensions: &'static [&'static str],
    /// Whether one line is a single-line comment in this dialect.
    pub is_line_comment: fn(&str) -> bool,
    /// Code identifiers, when this dialect can tell them from keywords, comments, and strings.
    pub identifiers: Option<fn(&str) -> Vec<String>>,
    /// Source with one code identifier renamed, when this dialect can tell it from keywords, comments, and strings.
    pub rename_identifier: Option<fn(&str, &str, &str) -> String>,
}

/// The Rust dialect.
///
/// Rust files use `.rs`, and a line comment begins with `//`, including the doc forms.
pub(crate) static RUST: Dialect = Dialect {
    extensions: &[".rs"],
    is_line_comment: rust_line_comment,
    identifiers: Some(rust::identifiers),
    rename_identifier: Some(rust::rename_identifier),
};

/// The Python dialect.
///
/// Python files use `.py`, and a line comment begins with `#`.
pub(crate) static PYTHON: Dialect = Dialect {
    extensions: &[".py"],
    is_line_comment: python_line_comment,
    identifiers: None,
    rename_identifier: None,
};

/// The dialect for a paired file, when semdiff knows that language.
///
/// Files with no dialect are skipped, so a comment marker in an unknown file is not reported.
pub(crate) fn for_path(path: &str) -> Option<&'static Dialect> {
    [&RUST, &PYTHON].into_iter().find(|dialect| {
        dialect
            .extensions
            .iter()
            .any(|extension| path.ends_with(extension))
    })
}

/// Whether a line is a Rust line comment.
///
/// Ordinary comments and both doc forms count, so an added `///` line is still reported.
fn rust_line_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Whether a line is a Python line comment.
///
/// A comment occupies the line, so a trailing note on a code line is not reported.
fn python_line_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::{PYTHON, RUST};

    #[test]
    fn rust_identifies_line_comments() {
        assert!(RUST.extensions.contains(&".rs"));
        assert!((RUST.is_line_comment)("// note"));
        assert!((RUST.is_line_comment)("  /// doc"));
        assert!((RUST.is_line_comment)("//! inner"));
        assert!(!(RUST.is_line_comment)("fn f() {} // trailing"));
    }

    #[test]
    fn python_identifies_line_comments() {
        assert!(PYTHON.extensions.contains(&".py"));
        assert!((PYTHON.is_line_comment)("# note"));
        assert!((PYTHON.is_line_comment)("  # indented"));
        assert!(!(PYTHON.is_line_comment)("def f():  # trailing"));
        assert!(!(PYTHON.is_line_comment)("\"\"\"doc\"\"\""));
    }
}
