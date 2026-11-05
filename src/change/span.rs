//! A change to a portion of one line.
//!
//! Directory, file, and line changes each have their own scope rules. A span
//! change names one old line, written `path:line`, or a run of old lines,
//! written `path:start-end`. The path is the file apply edits. An identifier
//! rename is the first span change. A later span change uses the same scope.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{applied_with_its_file, Change, Content, IdentifierRename, LineEdit, LinePlan};

use crate::filesystem::FileSystem;

/// The old lines a span change names.
///
/// The written scope is `path:line` for one line and `path:start-end` for a
/// run. `file` is the path apply edits. The lines are 1-based lines in the
/// old file.
pub(crate) struct SpanScope {
    scope: String,
    file: String,
    start: usize,
    end: usize,
}

impl SpanScope {
    /// A span on the old `line` of `path`.
    pub(crate) fn new(path: &str, line: usize) -> Self {
        Self::lines(path, line, line)
    }

    /// A span on the old lines `start` through `end` of `path`.
    ///
    /// One line is written `path:line`. A run is written `path:start-end`.
    fn lines(path: &str, start: usize, end: usize) -> Self {
        let scope = if start == end {
            format!("{path}:{start}")
        } else {
            format!("{path}:{start}-{end}")
        };
        Self {
            scope,
            file: path.to_string(),
            start,
            end,
        }
    }

    fn scope(&self) -> &str {
        &self.scope
    }

    fn file(&self) -> &str {
        &self.file
    }

    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }

    pub(crate) fn is_single(&self) -> bool {
        self.start == self.end
    }
}

/// The file and old lines named by a span scope.
///
/// The scope is `path:line` or `path:start-end`. The lines are 1-based lines
/// in the old file, and the end is not before the start. The last `:` separates
/// the path from the lines, so a colon in the path stays in the path.
pub(crate) fn parse_span_scope(scope: &str) -> Result<SpanScope, String> {
    let Some((path, lines)) = scope.rsplit_once(':') else {
        return Err(span_scope_error(scope));
    };
    if path.is_empty() {
        return Err(span_scope_error(scope));
    }
    let (start, end) = parse_line_span(lines).ok_or_else(|| span_scope_error(scope))?;
    Ok(SpanScope::lines(path, start, end))
}

fn parse_line_span(lines: &str) -> Option<(usize, usize)> {
    let (start, end) = if let Some((start, end)) = lines.split_once('-') {
        (line_number(start)?, line_number(end)?)
    } else {
        let line = line_number(lines)?;
        (line, line)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

fn line_number(text: &str) -> Option<usize> {
    match text.parse::<usize>() {
        Ok(number) if number >= 1 => Some(number),
        _ => None,
    }
}

fn span_scope_error(scope: &str) -> String {
    format!("span scope must be path:line or path:start-end, not {scope}")
}

/// An identifier rename on one or more old lines.
///
/// The scope is the span. Apply edits that path and replaces the old name on
/// those lines. `new` is that line in the new file when the rename is one
/// line. A merged run does not record `new`, because the lines do not share
/// one new line number.
pub(crate) struct IdentifierRenamed {
    span: SpanScope,
    new: Option<usize>,
    oldname: String,
    newname: String,
}

impl IdentifierRenamed {
    /// A rename of one identifier on the old line named by `span`.
    ///
    /// `new` is that line's number in the new file.
    pub(crate) fn new(span: SpanScope, new: usize, oldname: String, newname: String) -> Self {
        Self::recorded(span, Some(new), oldname, newname)
    }

    /// A rename read from a semdif record.
    ///
    /// `new` is present for one line and absent for a run.
    pub(crate) fn recorded(
        span: SpanScope,
        new: Option<usize>,
        oldname: String,
        newname: String,
    ) -> Self {
        Self {
            span,
            new,
            oldname,
            newname,
        }
    }

    fn merged(span: SpanScope, oldname: String, newname: String) -> Self {
        Self::recorded(span, None, oldname, newname)
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
        if let Some(new) = self.new {
            crate::semdif::write_plain(document, "new", &new.to_string());
        }
        crate::semdif::write_detail(document, "oldname", &self.oldname);
        crate::semdif::write_detail(document, "newname", &self.newname);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn identifier_rename(&self) -> Option<IdentifierRename<'_>> {
        Some(IdentifierRename {
            oldname: &self.oldname,
            newname: &self.newname,
            start: self.span.start(),
            end: self.span.end(),
        })
    }
}

impl Content for IdentifierRenamed {
    fn file(&self) -> &str {
        self.span.file()
    }
}

impl LineEdit for IdentifierRenamed {
    fn position(&self) -> (usize, usize) {
        (self.span.start(), self.new.unwrap_or(self.span.start()))
    }

    fn rebuild(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        let start = self.span.start();
        let end = self.span.end();
        if start == 0 || end < start || end > source_lines.len() {
            return Err(format!(
                "line {start}-{end} is outside {} which has {} lines",
                file.display(),
                source_lines.len()
            ));
        }
        if self.span.is_single() {
            let replaced = match crate::identifier::replace_name(
                self.span.file(),
                &source_lines[start - 1],
                &self.oldname,
                &self.newname,
            ) {
                Ok(text) => text,
                Err(message) => {
                    return Err(format!(
                        "cannot rename {} on line {start} in {}: {message}",
                        self.oldname,
                        file.display()
                    ));
                }
            };
            return plan.replace(file, start, replaced);
        }
        let mut found = false;
        for line_no in start..=end {
            let current = &source_lines[line_no - 1];
            let replaced = match crate::identifier::replace_every(
                self.span.file(),
                current,
                &self.oldname,
                &self.newname,
            ) {
                Ok(text) => text,
                Err(message) => {
                    return Err(format!(
                        "cannot rename {} on line {line_no} in {}: {message}",
                        self.oldname,
                        file.display()
                    ));
                }
            };
            if replaced != *current {
                found = true;
                plan.replace(file, line_no, replaced)?;
            }
        }
        if !found {
            return Err(format!(
                "cannot rename {} on lines {start}-{end} in {}: {} does not occur",
                self.oldname,
                file.display(),
                self.oldname
            ));
        }
        Ok(())
    }
}

/// Merges identifier renames of one name in one file.
///
/// Every rename with the same old name and new name is a candidate. Finding
/// those candidates does not require the lines to be adjacent. A candidate
/// merges with the next candidate of that name when every occurrence of the
/// old name between them is already one of those renames. The merged scope is
/// `path:start-end`.
pub(crate) fn merge_identifier_spans(
    path: &str,
    old_lines: &[String],
    changes: Vec<Box<dyn Change>>,
) -> Vec<Box<dyn Change>> {
    let mut groups: BTreeMap<(String, String), Vec<Candidate>> = BTreeMap::new();
    for (index, change) in changes.iter().enumerate() {
        let Some(rename) = change.identifier_rename() else {
            continue;
        };
        groups
            .entry((rename.oldname.to_string(), rename.newname.to_string()))
            .or_default()
            .push(Candidate {
                index,
                start: rename.start,
                end: rename.end,
            });
    }

    let mut replace = BTreeMap::new();
    let mut drop = BTreeSet::new();
    for ((oldname, newname), mut group) in groups {
        group.sort_by_key(|candidate| (candidate.start, candidate.end));
        let covered = covered_lines(&group);
        let mut runs = Vec::new();
        for candidate in &group {
            let extend = runs.last().is_some_and(|run: &Run| {
                candidate.start > run.end
                    && occurrences_covered(
                        path,
                        old_lines,
                        run.start,
                        candidate.end,
                        &oldname,
                        &covered,
                    )
            });
            if extend {
                let run = runs.last_mut().expect("the run was just found");
                run.end = candidate.end;
                run.rest.push(candidate.index);
            } else {
                runs.push(Run {
                    first: candidate.index,
                    start: candidate.start,
                    end: candidate.end,
                    rest: Vec::new(),
                });
            }
        }
        for run in runs {
            if run.rest.is_empty() || run.end <= run.start {
                continue;
            }
            replace.insert(
                run.first,
                Box::new(IdentifierRenamed::merged(
                    SpanScope::lines(path, run.start, run.end),
                    oldname.clone(),
                    newname.clone(),
                )) as Box<dyn Change>,
            );
            drop.extend(run.rest);
        }
    }
    if replace.is_empty() {
        return changes;
    }

    let mut merged = Vec::new();
    for (index, change) in changes.into_iter().enumerate() {
        if drop.contains(&index) {
            continue;
        }
        if let Some(span) = replace.remove(&index) {
            merged.push(span);
        } else {
            merged.push(change);
        }
    }
    merged
}

/// One identifier rename already in the change list.
///
/// `index` is its place in that list. `start` and `end` are the old lines it
/// already covers.
struct Candidate {
    index: usize,
    start: usize,
    end: usize,
}

/// A run of renames of one name that can share a scope.
struct Run {
    first: usize,
    start: usize,
    end: usize,
    rest: Vec<usize>,
}

fn covered_lines(group: &[Candidate]) -> BTreeSet<usize> {
    group
        .iter()
        .flat_map(|candidate| candidate.start..=candidate.end)
        .collect()
}

/// Whether every occurrence of `oldname` on `start` through `end` is a line
/// already renamed to that name.
fn occurrences_covered(
    path: &str,
    old_lines: &[String],
    start: usize,
    end: usize,
    oldname: &str,
    covered: &BTreeSet<usize>,
) -> bool {
    if start == 0 || end < start || end > old_lines.len() {
        return false;
    }
    for line_no in start..=end {
        let Some(count) =
            crate::identifier::occurrence_count(path, &old_lines[line_no - 1], oldname)
        else {
            return false;
        };
        if count == 0 {
            continue;
        }
        if count != 1 || !covered.contains(&line_no) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{merge_identifier_spans, parse_span_scope, IdentifierRenamed, SpanScope};
    use crate::change::Change;

    fn rename(line: usize, oldname: &str, newname: &str) -> Box<dyn Change> {
        Box::new(IdentifierRenamed::new(
            SpanScope::new("src/main.rs", line),
            line,
            oldname.to_string(),
            newname.to_string(),
        ))
    }

    #[test]
    fn a_span_scope_names_the_file_and_the_old_lines() {
        let scope = parse_span_scope("src/main.rs:37").unwrap();
        assert_eq!(scope.scope(), "src/main.rs:37");
        assert_eq!(scope.file(), "src/main.rs");
        assert_eq!(scope.start(), 37);
        assert_eq!(scope.end(), 37);

        let range = parse_span_scope("src/main.rs:35-38").unwrap();
        assert_eq!(range.scope(), "src/main.rs:35-38");
        assert_eq!(range.start(), 35);
        assert_eq!(range.end(), 38);

        let colon = parse_span_scope("weird:name.rs:4").unwrap();
        assert_eq!(colon.file(), "weird:name.rs");
        assert_eq!(colon.start(), 4);

        assert!(parse_span_scope("src/main.rs").is_err());
        assert!(parse_span_scope("src/main.rs:0").is_err());
        assert!(parse_span_scope("src/main.rs:46-37").is_err());
        assert!(parse_span_scope(":1").is_err());
    }

    #[test]
    fn renames_of_one_name_merge_across_a_gap_that_lacks_that_name() {
        let lines = vec![
            "foo".to_string(),
            "keep".to_string(),
            "foo".to_string(),
            "keep".to_string(),
            "foo".to_string(),
        ];
        let changes = vec![
            rename(1, "foo", "bar"),
            rename(3, "foo", "bar"),
            rename(5, "foo", "bar"),
        ];
        let merged = merge_identifier_spans("src/main.rs", &lines, changes);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].scope(), "src/main.rs:1-5");
    }

    #[test]
    fn an_uncovered_name_between_renames_keeps_them_apart() {
        let lines = vec!["foo".to_string(), "foo".to_string(), "foo".to_string()];
        let changes = vec![rename(1, "foo", "bar"), rename(3, "foo", "bar")];
        let merged = merge_identifier_spans("src/main.rs", &lines, changes);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].scope(), "src/main.rs:1");
        assert_eq!(merged[1].scope(), "src/main.rs:3");
    }

    #[test]
    fn a_different_new_name_stays_apart() {
        let lines = vec!["foo".to_string(), "foo".to_string()];
        let changes = vec![rename(1, "foo", "bar"), rename(2, "foo", "baz")];
        let merged = merge_identifier_spans("src/main.rs", &lines, changes);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].scope(), "src/main.rs:1");
        assert_eq!(merged[1].scope(), "src/main.rs:2");
    }
}
