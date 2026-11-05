//! A change to a portion of one line.
//!
//! Directory, file, and line changes each have their own scope rules. A span
//! change names one old line, written `path:line`. The path is the file apply
//! edits. An identifier rename is the first span change. A later span change
//! uses the same scope.

use std::path::Path;

use super::{applied_with_its_file, Change, Content, LineEdit, LinePlan};

use crate::filesystem::FileSystem;

/// The old line a span change names.
///
/// The written scope is `path:line`. `file` is the path apply edits, and
/// `line` is the 1-based line in the old file.
pub(crate) struct SpanScope {
    scope: String,
    file: String,
    line: usize,
}

impl SpanScope {
    /// A span on the old `line` of `path`.
    pub(crate) fn new(path: &str, line: usize) -> Self {
        Self {
            scope: format!("{path}:{line}"),
            file: path.to_string(),
            line,
        }
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn file(&self) -> &str {
        &self.file
    }

    fn line(&self) -> usize {
        self.line
    }
}

/// The file and old line named by a span scope.
///
/// The scope is `path:line`. The line is a 1-based line in the old file.
/// The last `:` separates them, so a colon in the path stays in the path.
/// A range is not one line.
pub(crate) fn parse_span_scope(scope: &str) -> Result<SpanScope, String> {
    let Some((path, line)) = scope.rsplit_once(':') else {
        return Err(span_scope_error(scope));
    };
    if path.is_empty() {
        return Err(span_scope_error(scope));
    }
    match line.parse::<usize>() {
        Ok(number) if number >= 1 => Ok(SpanScope::new(path, number)),
        _ => Err(span_scope_error(scope)),
    }
}

fn span_scope_error(scope: &str) -> String {
    format!("span scope must be path:line, not {scope}")
}

/// An identifier rename on one old line.
///
/// The scope is the span's `path:line`. Apply edits that path and replaces
/// the one occurrence of the old name on that line. `new` is the same line in
/// the new file. A rename does not add or remove lines, so that number can
/// move when an earlier line change shifts the file. It stays in the details
/// until a later step derives it.
pub(crate) struct IdentifierRenamed {
    span: SpanScope,
    new: usize,
    oldname: String,
    newname: String,
}

impl IdentifierRenamed {
    /// A rename of one identifier on the old line named by `span`.
    ///
    /// `new` is that line's number in the new file.
    pub(crate) fn new(span: SpanScope, new: usize, oldname: String, newname: String) -> Self {
        Self {
            span,
            new,
            oldname,
            newname,
        }
    }
}

impl Change for IdentifierRenamed {
    fn kind(&self) -> &'static str {
        "identifier-renamed"
    }

    fn scope(&self) -> &str {
        self.span.scope()
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        Some(self)
    }

    fn write_details(&self, document: &mut String) {
        crate::semdif::write_plain(document, "new", &self.new.to_string());
        crate::semdif::write_detail(document, "oldname", &self.oldname);
        crate::semdif::write_detail(document, "newname", &self.newname);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }
}

impl Content for IdentifierRenamed {
    fn file(&self) -> &str {
        self.span.file()
    }
}

impl LineEdit for IdentifierRenamed {
    fn position(&self) -> (usize, usize) {
        (self.span.line(), self.new)
    }

    fn rebuild(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        let old = self.span.line();
        if old == 0 || old > source_lines.len() {
            return Err(format!(
                "line {old} is outside {} which has {} lines",
                file.display(),
                source_lines.len()
            ));
        }
        let replaced = match crate::identifier::replace_name(
            self.span.file(),
            &source_lines[old - 1],
            &self.oldname,
            &self.newname,
        ) {
            Ok(text) => text,
            Err(message) => {
                return Err(format!(
                    "cannot rename {} on line {old} in {}: {message}",
                    self.oldname,
                    file.display()
                ));
            }
        };
        plan.replace(file, old, replaced)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_span_scope;

    #[test]
    fn a_span_scope_names_the_file_and_the_old_line() {
        let scope = parse_span_scope("src/main.rs:37").unwrap();
        assert_eq!(scope.scope(), "src/main.rs:37");
        assert_eq!(scope.file(), "src/main.rs");
        assert_eq!(scope.line(), 37);

        let colon = parse_span_scope("weird:name.rs:4").unwrap();
        assert_eq!(colon.file(), "weird:name.rs");
        assert_eq!(colon.line(), 4);

        assert!(parse_span_scope("src/main.rs").is_err());
        assert!(parse_span_scope("src/main.rs:0").is_err());
        assert!(parse_span_scope("src/main.rs:37-46").is_err());
        assert!(parse_span_scope(":1").is_err());
    }
}
