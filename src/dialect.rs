/// A language semdiff knows how to read.
///
/// Extensions select its files, and the line-comment test selects its comment lines, so
/// another language can be added without changing the comparison.
pub(crate) struct Dialect {
    /// Extensions that identify this dialect, including the leading dot.
    pub extensions: &'static [&'static str],
    /// Whether one line is a single-line comment in this dialect.
    pub is_line_comment: fn(&str) -> bool,
}

/// The Rust dialect.
///
/// Rust files use `.rs`, and a line comment begins with `//`, including the doc forms.
pub(crate) static RUST: Dialect = Dialect {
    extensions: &[".rs"],
    is_line_comment: rust_line_comment,
};

/// The dialect for a paired file, when semdiff knows that language.
///
/// Files with no dialect are skipped, so a comment marker in an unknown file is not reported.
pub(crate) fn for_path(path: &str) -> Option<&'static Dialect> {
    [&RUST].into_iter().find(|dialect| {
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

#[cfg(test)]
mod tests {
    use super::RUST;

    #[test]
    fn rust_identifies_line_comments() {
        assert!(RUST.extensions.contains(&".rs"));
        assert!((RUST.is_line_comment)("// note"));
        assert!((RUST.is_line_comment)("  /// doc"));
        assert!((RUST.is_line_comment)("//! inner"));
        assert!(!(RUST.is_line_comment)("fn f() {} // trailing"));
    }
}
