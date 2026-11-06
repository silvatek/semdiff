//! A change to a portion of one line.
//!
//! Directory, file, and line changes each have their own scope rules. A span
//! change names one old line, written `path:line`, or a run of old lines,
//! written `path:start-end`. The path is the file apply edits. An identifier
//! rename is the first span change. A later span change uses the same scope.
//! A rename of one name across files drops the line numbers and names the
//! directory that covers those files.

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

/// A parsed identifier-rename scope.
///
/// A line scope names old lines of one file. A directory scope names the
/// directory that covers several files and has no line numbers.
pub(crate) enum RenameScope {
    Lines(SpanScope),
    Directory(String),
}

/// Reads an identifier-rename scope.
///
/// `path:line` and `path:start-end` are line spans. A scope with no line
/// numbers is the directory those files share, including `.` for the whole
/// tree. A line number that is not a real line is an error.
pub(crate) fn parse_rename_scope(scope: &str) -> Result<RenameScope, String> {
    if scope.is_empty() {
        return Err(span_scope_error(scope));
    }
    if let Some((_, lines)) = scope.rsplit_once(':') {
        if is_line_shaped(lines) {
            return Ok(RenameScope::Lines(parse_span_scope(scope)?));
        }
    }
    Ok(RenameScope::Directory(scope.to_string()))
}

fn is_line_shaped(lines: &str) -> bool {
    let mut parts = lines.split('-');
    let Some(start) = parts.next() else {
        return false;
    };
    if !is_digits(start) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(end) => parts.next().is_none() && is_digits(end),
    }
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| character.is_ascii_digit())
}

/// Where an identifier rename applies.
///
/// A line span names old lines of one file. A directory names every file under
/// that path and records no line numbers.
enum Extent {
    Lines { span: SpanScope, new: Option<usize> },
    Directory { scope: String },
}

/// An identifier rename on one or more old lines, or across the files in a directory.
///
/// A line span's scope is that span. Apply edits that path and replaces the
/// old name on those lines. `new` is that line in the new file when the rename
/// is one line. A merged run does not record `new`, because the lines do not
/// share one new line number. A directory scope names the directory that
/// covers every file in the rename, and apply replaces the name in each of
/// those files.
pub(crate) struct IdentifierRenamed {
    extent: Extent,
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
            extent: Extent::Lines { span, new },
            oldname,
            newname,
        }
    }

    /// A rename of one name in every file under `scope`.
    ///
    /// The scope is a directory, or `.` for the whole tree. It records no line
    /// numbers.
    pub(crate) fn directory(scope: String, oldname: String, newname: String) -> Self {
        Self {
            extent: Extent::Directory { scope },
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
        match &self.extent {
            Extent::Lines { span, .. } => span.scope(),
            Extent::Directory { scope } => scope,
        }
    }

    fn line_edit(&self) -> Option<&dyn LineEdit> {
        Some(self)
    }

    fn content(&self) -> Option<&dyn Content> {
        match &self.extent {
            Extent::Lines { .. } => Some(self),
            Extent::Directory { .. } => None,
        }
    }

    fn write_details(&self, document: &mut String) {
        if let Extent::Lines { new: Some(new), .. } = &self.extent {
            crate::semdif::write_plain(document, "new", &new.to_string());
        }
        crate::semdif::write_detail(document, "oldname", &self.oldname);
        crate::semdif::write_detail(document, "newname", &self.newname);
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn identifier_rename(&self) -> Option<IdentifierRename<'_>> {
        let Extent::Lines { span, .. } = &self.extent else {
            return None;
        };
        Some(IdentifierRename {
            file: span.file(),
            oldname: &self.oldname,
            newname: &self.newname,
            start: span.start(),
            end: span.end(),
        })
    }

    fn edits_file(&self, path: &str) -> bool {
        match &self.extent {
            Extent::Lines { span, .. } => span.file() == path,
            Extent::Directory { scope } => covers(scope, path),
        }
    }

    fn directory_scope(&self) -> Option<&str> {
        match &self.extent {
            Extent::Directory { scope } => Some(scope),
            Extent::Lines { .. } => None,
        }
    }
}

impl Content for IdentifierRenamed {
    fn file(&self) -> &str {
        match &self.extent {
            Extent::Lines { span, .. } => span.file(),
            Extent::Directory { .. } => "",
        }
    }
}

impl LineEdit for IdentifierRenamed {
    fn position(&self) -> (usize, usize) {
        match &self.extent {
            Extent::Lines { span, new } => (span.start(), new.unwrap_or(span.start())),
            Extent::Directory { .. } => (0, 0),
        }
    }

    fn rebuild(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        match &self.extent {
            Extent::Directory { .. } => self.rebuild_directory(file, source_lines, plan),
            Extent::Lines { span, .. } => self.rebuild_lines(file, span, source_lines, plan),
        }
    }
}

impl IdentifierRenamed {
    /// Replaces the old name on each line of one file under a directory scope.
    ///
    /// A file with no dialect is left alone. A line that does not contain the
    /// name is left alone. The file's other line edits still run.
    fn rebuild_directory(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        let path = file.to_string_lossy();
        if crate::dialect::for_path(&path).is_none() {
            return Ok(());
        }
        for line_no in 1..=source_lines.len() {
            let current = &source_lines[line_no - 1];
            let replaced = match crate::identifier::replace_every(
                &path,
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
                plan.replace(file, line_no, replaced)?;
            }
        }
        Ok(())
    }

    /// Replaces the old name on the old lines named by a line span.
    fn rebuild_lines(
        &self,
        file: &Path,
        span: &SpanScope,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        let start = span.start();
        let end = span.end();
        if start == 0 || end < start || end > source_lines.len() {
            return Err(format!(
                "line {start}-{end} is outside {} which has {} lines",
                file.display(),
                source_lines.len()
            ));
        }
        if span.is_single() {
            let replaced = match crate::identifier::replace_name(
                span.file(),
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
                span.file(),
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

/// An integer constant changed on one old line.
///
/// The scope is that line. Apply replaces one digit sequence there. `new` is
/// that line in the new file. `occurrence` is the 1-based index of the
/// sequence among the digit sequences on the old line.
pub(crate) struct ConstantChanged {
    span: SpanScope,
    new_line: usize,
    old_value: String,
    new_value: String,
    occurrence: usize,
}

impl ConstantChanged {
    /// A change of one integer on the old line named by `span`.
    ///
    /// `new_line` is that line's number in the new file. `occurrence` selects
    /// the digit sequence when the line has several.
    pub(crate) fn new(
        span: SpanScope,
        new_line: usize,
        old_value: String,
        new_value: String,
        occurrence: usize,
    ) -> Self {
        Self {
            span,
            new_line,
            old_value,
            new_value,
            occurrence,
        }
    }
}

impl Change for ConstantChanged {
    fn kind(&self) -> &'static str {
        "constant-changed"
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
        crate::semdif::write_plain(document, "new", &self.new_line.to_string());
        crate::semdif::write_detail(document, "oldvalue", &self.old_value);
        crate::semdif::write_detail(document, "newvalue", &self.new_value);
        crate::semdif::write_plain(document, "occurrence", &self.occurrence.to_string());
    }

    fn apply(&self, _fs: &dyn FileSystem, _source: &Path) -> Result<(), String> {
        applied_with_its_file(self.kind())
    }

    fn edits_file(&self, path: &str) -> bool {
        self.span.file() == path
    }
}

impl Content for ConstantChanged {
    fn file(&self) -> &str {
        self.span.file()
    }
}

impl LineEdit for ConstantChanged {
    fn position(&self) -> (usize, usize) {
        (self.span.start(), self.new_line)
    }

    fn rebuild(
        &self,
        file: &Path,
        source_lines: &[String],
        plan: &mut LinePlan,
    ) -> Result<(), String> {
        let line_no = self.span.start();
        if line_no == 0 || line_no != self.span.end() || line_no > source_lines.len() {
            return Err(format!(
                "line {line_no} is outside {} which has {} lines",
                file.display(),
                source_lines.len()
            ));
        }
        let replaced = match crate::constant::replace_integer(
            self.span.file(),
            &source_lines[line_no - 1],
            &self.old_value,
            &self.new_value,
            self.occurrence,
        ) {
            Ok(text) => text,
            Err(message) => {
                return Err(format!(
                    "cannot change integer {} on line {line_no} in {}: {message}",
                    self.old_value,
                    file.display()
                ));
            }
        };
        plan.replace(file, line_no, replaced)
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

/// One old file's text, used to see where a name still occurs.
///
/// The lines are the old file. A file that is not text has no lines, so it
/// holds no identifiers.
pub(crate) struct SourceText {
    pub(crate) path: String,
    pub(crate) lines: Vec<String>,
}

/// Merges identifier renames of one name across files.
///
/// Renames inside one file are already merged. Every remaining rename with the
/// same old name and new name is a candidate, in path order. Finding those
/// candidates does not require the files to be adjacent. A candidate joins the
/// next candidate when every occurrence of the old name in the directory that
/// covers both is already one of those renames. The merged scope is that
/// directory, with no line numbers.
pub(crate) fn merge_identifier_files(
    sources: &[SourceText],
    changes: Vec<Box<dyn Change>>,
) -> Vec<Box<dyn Change>> {
    let mut groups: BTreeMap<(String, String), Vec<FileCandidate>> = BTreeMap::new();
    for (index, change) in changes.iter().enumerate() {
        let Some(rename) = change.identifier_rename() else {
            continue;
        };
        groups
            .entry((rename.oldname.to_string(), rename.newname.to_string()))
            .or_default()
            .push(FileCandidate {
                index,
                file: rename.file.to_string(),
                start: rename.start,
                end: rename.end,
            });
    }

    let mut replace = BTreeMap::new();
    let mut drop = BTreeSet::new();
    for ((oldname, newname), mut group) in groups {
        group.sort_by_key(|candidate| (candidate.file.clone(), candidate.start, candidate.end));
        let covered = covered_file_lines(&group);
        let mut runs = Vec::new();
        for candidate in &group {
            let directory = runs.last().and_then(|run: &FileRun| {
                if run.files.last() == Some(&candidate.file) {
                    return None;
                }
                let mut paths: Vec<&str> = run.files.iter().map(String::as_str).collect();
                paths.push(&candidate.file);
                let directory = common_directory(&paths);
                directory_occurrences_covered(sources, &directory, &oldname, &covered)
                    .then_some(directory)
            });
            if let Some(directory) = directory {
                let run = runs.last_mut().expect("the run was just found");
                run.directory = directory;
                run.files.push(candidate.file.clone());
                run.rest.push(candidate.index);
            } else {
                runs.push(FileRun {
                    first: candidate.index,
                    directory: String::new(),
                    files: vec![candidate.file.clone()],
                    rest: Vec::new(),
                });
            }
        }
        for run in runs {
            if run.rest.is_empty() {
                continue;
            }
            replace.insert(
                run.first,
                Box::new(IdentifierRenamed::directory(
                    run.directory,
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

/// One identifier rename in a file, already merged with the other lines of that file.
struct FileCandidate {
    index: usize,
    file: String,
    start: usize,
    end: usize,
}

/// A run of renames of one name that can share a directory scope.
struct FileRun {
    first: usize,
    directory: String,
    files: Vec<String>,
    rest: Vec<usize>,
}

fn covered_file_lines(group: &[FileCandidate]) -> BTreeSet<(String, usize)> {
    let mut covered = BTreeSet::new();
    for candidate in group {
        for line in candidate.start..=candidate.end {
            covered.insert((candidate.file.clone(), line));
        }
    }
    covered
}

/// The directory that contains every path.
///
/// The directory is the parents' common prefix. Files at the top of the tree
/// share `.`.
fn common_directory(paths: &[&str]) -> String {
    let mut common: Option<Vec<&str>> = None;
    for path in paths {
        let parent = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
        let parts: Vec<&str> = if parent.is_empty() {
            Vec::new()
        } else {
            parent.split('/').collect()
        };
        common = Some(match common {
            None => parts,
            Some(existing) => existing
                .into_iter()
                .zip(parts)
                .take_while(|(left, right)| left == right)
                .map(|(left, _)| left)
                .collect(),
        });
    }
    match common {
        Some(parts) if !parts.is_empty() => parts.join("/"),
        _ => super::TREE_SCOPE.to_string(),
    }
}

fn covers(directory: &str, file: &str) -> bool {
    if directory == super::TREE_SCOPE {
        return true;
    }
    file.strip_prefix(directory)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether every occurrence of `oldname` under `directory` is a line already renamed to that name.
fn directory_occurrences_covered(
    sources: &[SourceText],
    directory: &str,
    oldname: &str,
    covered: &BTreeSet<(String, usize)>,
) -> bool {
    for source in sources {
        if !covers(directory, &source.path) {
            continue;
        }
        for (index, line) in source.lines.iter().enumerate() {
            let line_no = index + 1;
            let Some(count) = crate::identifier::occurrence_count(&source.path, line, oldname)
            else {
                continue;
            };
            if count == 0 {
                continue;
            }
            if count != 1 || !covered.contains(&(source.path.clone(), line_no)) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        merge_identifier_files, merge_identifier_spans, parse_rename_scope, parse_span_scope,
        IdentifierRenamed, RenameScope, SourceText, SpanScope,
    };
    use crate::change::Change;

    fn rename(line: usize, oldname: &str, newname: &str) -> Box<dyn Change> {
        rename_at("src/main.rs", line, oldname, newname)
    }

    fn rename_at(path: &str, line: usize, oldname: &str, newname: &str) -> Box<dyn Change> {
        Box::new(IdentifierRenamed::new(
            SpanScope::new(path, line),
            line,
            oldname.to_string(),
            newname.to_string(),
        ))
    }

    fn source(path: &str, text: &str) -> SourceText {
        SourceText {
            path: path.to_string(),
            lines: text
                .strip_suffix('\n')
                .unwrap_or(text)
                .split('\n')
                .map(str::to_string)
                .collect(),
        }
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

    #[test]
    fn a_directory_scope_has_no_line_numbers() {
        match parse_rename_scope("src").unwrap() {
            RenameScope::Directory(scope) => assert_eq!(scope, "src"),
            RenameScope::Lines(_) => panic!("src is a directory"),
        }
        match parse_rename_scope(".").unwrap() {
            RenameScope::Directory(scope) => assert_eq!(scope, "."),
            RenameScope::Lines(_) => panic!(". is the tree"),
        }
        assert!(parse_rename_scope("src/main.rs:0").is_err());
        assert!(matches!(
            parse_rename_scope("src/main.rs:4").unwrap(),
            RenameScope::Lines(_)
        ));
    }

    #[test]
    fn renames_in_one_directory_merge_when_that_directory_is_covered() {
        let sources = vec![
            source("src/a.rs", "foo\n"),
            source("src/b.rs", "foo\n"),
            source("tests/c.rs", "foo\n"),
        ];
        let changes = vec![
            rename_at("src/a.rs", 1, "foo", "bar"),
            rename_at("src/b.rs", 1, "foo", "bar"),
        ];
        let merged = merge_identifier_files(&sources, changes);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].scope(), "src");
    }

    #[test]
    fn an_uncovered_name_in_the_directory_keeps_the_files_apart() {
        let sources = vec![
            source("src/a.rs", "foo\n"),
            source("src/b.rs", "foo\n"),
            source("src/c.rs", "foo\n"),
        ];
        let changes = vec![
            rename_at("src/a.rs", 1, "foo", "bar"),
            rename_at("src/b.rs", 1, "foo", "bar"),
        ];
        let merged = merge_identifier_files(&sources, changes);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].scope(), "src/a.rs:1");
        assert_eq!(merged[1].scope(), "src/b.rs:1");
    }

    #[test]
    fn a_covered_directory_merges_and_a_later_file_stays_apart() {
        let sources = vec![
            source("other/c.rs", "foo\n"),
            source("other/d.rs", "foo\n"),
            source("src/a.rs", "foo\n"),
            source("src/b.rs", "foo\n"),
        ];
        let changes = vec![
            rename_at("other/c.rs", 1, "foo", "bar"),
            rename_at("src/a.rs", 1, "foo", "bar"),
            rename_at("src/b.rs", 1, "foo", "bar"),
        ];
        let merged = merge_identifier_files(&sources, changes);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].scope(), "other/c.rs:1");
        assert_eq!(merged[1].scope(), "src");
    }
}
