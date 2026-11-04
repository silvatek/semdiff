use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::Range;
use std::path::Path;

use crate::dialect::{Span, SpanKind};

/// A symbol rename that covers every use in one scope.
///
/// The scope is the smallest file or directory that contains those uses, and every
/// use of the old name there changed to the same new name.
pub(crate) struct SymbolRename {
    pub scope: String,
    pub from: String,
    pub to: String,
}

/// One paired file whose spans have been classified.
///
/// The comparison has already read the file. Symbol renames are decided from these
/// texts, including a file whose text did not change, because a remaining use counts.
pub(crate) struct ComparedFile {
    path: String,
    left: String,
    right: String,
    left_spans: Vec<Span>,
    right_spans: Vec<Span>,
}

/// Classifies the identifiers in a paired file that has already been read.
pub(crate) fn compared_file(
    path: &str,
    left: String,
    right: String,
    classify: fn(&str) -> Vec<Span>,
) -> ComparedFile {
    let left_spans = classify(&left);
    let right_spans = classify(&right);
    ComparedFile {
        path: path.to_string(),
        left,
        right,
        left_spans,
        right_spans,
    }
}

impl ComparedFile {
    /// The paired file this comparison came from.
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    /// Whether this file uses `name` as a code identifier on either side.
    ///
    /// A wider scope cannot be chosen while a use remains outside the renamed files.
    pub(crate) fn has_use(&self, name: &str) -> bool {
        identifier_uses(&self.left, &self.left_spans, name) > 0
            || identifier_uses(&self.right, &self.right_spans, name) > 0
    }
}

/// Symbol renames that are complete in one file.
///
/// The file is the scope. The same rename in other files can later share a wider
/// scope when that scope still covers every use. A comment or a string is not a use.
pub(crate) fn file_symbol_renames(file: &ComparedFile) -> Vec<SymbolRename> {
    let edits = if file.left != file.right {
        identifier_edits(&file.left, &file.right, &file.left_spans, &file.right_spans)
    } else {
        Vec::new()
    };
    renames_for_scope(std::slice::from_ref(file), &edits)
}

/// Renames one code identifier in every file of `scope`.
///
/// A dialect classifies each span, so a keyword, comment, or string is left unchanged.
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

#[derive(Clone, Copy)]
struct TextLine<'a> {
    start: usize,
    text: &'a str,
}

/// One rename for each name whose every use in these files changed to the same new name.
///
/// The scope is the smallest file or directory that contains those uses. A name that
/// is still used there, or that changed to more than one new name, is not recorded.
fn renames_for_scope(files: &[ComparedFile], edits: &[(String, String)]) -> Vec<SymbolRename> {
    let mut by_from: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for (from, to) in edits {
        *by_from.entry(from).or_default().entry(to).or_default() += 1;
    }
    let mut renames = Vec::new();
    for (from, replacements) in by_from {
        let Some((to, change_count)) = single_replacement(&replacements) else {
            continue;
        };
        let left_uses = files
            .iter()
            .map(|file| identifier_uses(&file.left, &file.left_spans, from))
            .sum::<usize>();
        let right_uses = files
            .iter()
            .map(|file| identifier_uses(&file.right, &file.right_spans, from))
            .sum::<usize>();
        let use_paths: Vec<&str> = files
            .iter()
            .filter(|file| {
                identifier_uses(&file.left, &file.left_spans, from) > 0
                    || identifier_uses(&file.right, &file.right_spans, from) > 0
            })
            .map(|file| file.path.as_str())
            .collect();
        if change_count == left_uses && right_uses == 0 && !use_paths.is_empty() {
            renames.push(SymbolRename {
                scope: tightest_scope(&use_paths),
                from: from.to_string(),
                to: to.to_string(),
            });
        }
    }
    renames
}

/// The smallest file or directory that contains every path.
///
/// One file is its own scope. Several files share the longest directory that still
/// contains them, and the compared tree is `.` when they share no directory.
pub(crate) fn tightest_scope(paths: &[&str]) -> String {
    let Some(first) = paths.first() else {
        return ".".to_string();
    };
    if paths.len() == 1 {
        return (*first).to_string();
    }
    let mut common = directory(first);
    for path in &paths[1..] {
        let shared = common
            .iter()
            .zip(directory(path))
            .take_while(|(left, right)| *left == right)
            .count();
        common.truncate(shared);
    }
    if common.is_empty() {
        ".".to_string()
    } else {
        common.join("/")
    }
}

fn directory(path: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    parts
}

/// The new name, when every change of an identifier chose that same name.
fn single_replacement<'a>(replacements: &BTreeMap<&'a str, usize>) -> Option<(&'a str, usize)> {
    if replacements.len() != 1 {
        return None;
    }
    replacements.iter().next().map(|(to, count)| (*to, *count))
}

/// How many times `name` is used as a code identifier.
fn identifier_uses(source: &str, spans: &[Span], name: &str) -> usize {
    spans
        .iter()
        .filter(|span| span.kind == SpanKind::Identifier && &source[span.start..span.end] == name)
        .count()
}

/// Identifier spans whose text changed between two versions of a file.
///
/// A comment or a string can repeat a name without that being a use of the identifier.
fn identifier_edits(
    left: &str,
    right: &str,
    left_spans: &[Span],
    right_spans: &[Span],
) -> Vec<(String, String)> {
    let left_code = code_text(left, left_spans);
    let right_code = code_text(right, right_spans);
    let mut edits = Vec::new();
    let mut seen = BTreeSet::new();
    for (left_line, right_line) in changed_line_pairs(left, right) {
        let left_text = &left_code[left_line.start..left_line.start + left_line.text.len()];
        let right_text = &right_code[right_line.start..right_line.start + right_line.text.len()];
        for (left_range, right_range) in changed_ranges(left_text, right_text) {
            let left_token = changed_token(
                left_spans,
                left_line.start + left_range.start,
                left_line.start + left_range.end,
            );
            let right_token = changed_token(
                right_spans,
                right_line.start + right_range.start,
                right_line.start + right_range.end,
            );
            let (Some(left_token), Some(right_token)) = (left_token, right_token) else {
                continue;
            };
            if left_token.kind != SpanKind::Identifier || right_token.kind != SpanKind::Identifier {
                continue;
            }
            let from = left[left_token.start..left_token.end].to_string();
            let to = right[right_token.start..right_token.end].to_string();
            if from != to && seen.insert((left_token.start, right_token.start)) {
                edits.push((from, to));
            }
        }
    }
    edits
}

/// The file text that is code.
///
/// A comment or string is not part of the code, so a repeated name there is not a use.
fn code_text(source: &str, spans: &[Span]) -> String {
    let mut bytes = source.as_bytes().to_vec();
    for span in spans {
        if matches!(span.kind, SpanKind::Comment | SpanKind::String) {
            for byte in &mut bytes[span.start..span.end] {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_string())
}

/// The span that contains a changed region of a line.
///
/// A deletion that ends where an identifier ends still belongs to that identifier.
fn changed_token(spans: &[Span], start: usize, end: usize) -> Option<&Span> {
    if start < end {
        return spans
            .iter()
            .find(|span| span.start <= start && start < span.end);
    }
    spans.iter().find(|span| {
        span.end == start && matches!(span.kind, SpanKind::Identifier | SpanKind::Keyword)
    })
}

/// Lines whose text differs, paired when they are the same line changed.
///
/// An inserted comment is not paired with a code line.
fn changed_line_pairs<'a>(left: &'a str, right: &'a str) -> Vec<(TextLine<'a>, TextLine<'a>)> {
    let left_lines = file_lines(left);
    let right_lines = file_lines(right);
    let ops = diff_lines(&left_lines, &right_lines);
    let mut pairs = Vec::new();
    let mut deleted = Vec::new();
    let mut inserted = Vec::new();
    let flush = |deleted: &mut Vec<TextLine<'a>>,
                 inserted: &mut Vec<TextLine<'a>>,
                 pairs: &mut Vec<(TextLine<'a>, TextLine<'a>)>| {
        pairs.extend(pair_changed_lines(deleted, inserted));
        deleted.clear();
        inserted.clear();
    };
    for op in ops {
        match op {
            LineOp::Equal => flush(&mut deleted, &mut inserted, &mut pairs),
            LineOp::Delete(line) => deleted.push(line),
            LineOp::Insert(line) => inserted.push(line),
        }
    }
    flush(&mut deleted, &mut inserted, &mut pairs);
    pairs
}

/// Pairs the removed and added lines of one region that are the same line changed.
fn pair_changed_lines<'a>(
    deleted: &[TextLine<'a>],
    inserted: &[TextLine<'a>],
) -> Vec<(TextLine<'a>, TextLine<'a>)> {
    let mut candidates = Vec::new();
    for (deleted_index, deleted_line) in deleted.iter().enumerate() {
        for (inserted_index, inserted_line) in inserted.iter().enumerate() {
            let score = line_similarity(deleted_line.text, inserted_line.text);
            if score >= 0.5 {
                candidates.push((score, deleted_index, inserted_index));
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut used_deleted = vec![false; deleted.len()];
    let mut used_inserted = vec![false; inserted.len()];
    let mut pairs = Vec::new();
    for (_, deleted_index, inserted_index) in candidates {
        if used_deleted[deleted_index] || used_inserted[inserted_index] {
            continue;
        }
        used_deleted[deleted_index] = true;
        used_inserted[inserted_index] = true;
        pairs.push((deleted[deleted_index], inserted[inserted_index]));
    }
    pairs
}

/// How much of two lines is the same text.
///
/// A renamed line still shares most of its text, and an unrelated inserted line does not.
fn line_similarity(left: &str, right: &str) -> f64 {
    let shared = shared_bytes(left.as_bytes(), right.as_bytes());
    shared as f64 / left.len().max(right.len()).max(1) as f64
}

/// How much text two lines have in common.
fn shared_bytes(left: &[u8], right: &[u8]) -> usize {
    let mut previous = vec![0usize; right.len() + 1];
    let mut current = vec![0usize; right.len() + 1];
    for left_byte in left {
        for (index, right_byte) in right.iter().enumerate() {
            current[index + 1] = if left_byte == right_byte {
                previous[index] + 1
            } else {
                current[index].max(previous[index + 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }
    previous[right.len()]
}

/// The lines of a file, each with its offset in the file.
///
/// A span is located in the whole file, so a line needs to know where it starts.
fn file_lines(source: &str) -> Vec<TextLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, character) in source.char_indices() {
        if character == '\n' {
            lines.push(TextLine {
                start,
                text: &source[start..index],
            });
            start = index + character.len_utf8();
        }
    }
    if start < source.len() {
        lines.push(TextLine {
            start,
            text: &source[start..],
        });
    }
    lines
}

enum LineOp<'a> {
    Equal,
    Delete(TextLine<'a>),
    Insert(TextLine<'a>),
}

/// Which lines were kept, removed, or added.
///
/// Kept lines separate the regions where a line may have changed.
fn diff_lines<'a>(left: &[TextLine<'a>], right: &[TextLine<'a>]) -> Vec<LineOp<'a>> {
    let left_len = left.len();
    let right_len = right.len();
    let mut common = vec![vec![0usize; right_len + 1]; left_len + 1];
    for left_index in (0..left_len).rev() {
        for right_index in (0..right_len).rev() {
            common[left_index][right_index] = if left[left_index].text == right[right_index].text {
                common[left_index + 1][right_index + 1] + 1
            } else {
                common[left_index + 1][right_index].max(common[left_index][right_index + 1])
            };
        }
    }
    let mut ops = Vec::new();
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left_len && right_index < right_len {
        if left[left_index].text == right[right_index].text
            && common[left_index][right_index] == common[left_index + 1][right_index + 1] + 1
        {
            ops.push(LineOp::Equal);
            left_index += 1;
            right_index += 1;
        } else if common[left_index + 1][right_index] >= common[left_index][right_index + 1] {
            ops.push(LineOp::Delete(left[left_index]));
            left_index += 1;
        } else {
            ops.push(LineOp::Insert(right[right_index]));
            right_index += 1;
        }
    }
    while left_index < left_len {
        ops.push(LineOp::Delete(left[left_index]));
        left_index += 1;
    }
    while right_index < right_len {
        ops.push(LineOp::Insert(right[right_index]));
        right_index += 1;
    }
    ops
}

/// The regions of two lines that are not the same text.
///
/// The shared start and end of the line stay put, so a change is the text between them.
fn changed_ranges(left: &str, right: &str) -> Vec<(Range<usize>, Range<usize>)> {
    let prefix = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    let left_end = left.len() - prefix;
    let right_end = right.len() - prefix;
    let suffix = left[prefix..]
        .bytes()
        .rev()
        .zip(right[prefix..].bytes().rev())
        .take_while(|(left, right)| left == right)
        .count()
        .min(left_end)
        .min(right_end);
    let mut ranges = changed_middle(
        &left[prefix..left.len() - suffix],
        &right[prefix..right.len() - suffix],
    );
    for (left_range, right_range) in &mut ranges {
        *left_range = (left_range.start + prefix)..(left_range.end + prefix);
        *right_range = (right_range.start + prefix)..(right_range.end + prefix);
    }
    ranges
}

/// The regions of two texts that are not the same.
fn changed_middle(left: &str, right: &str) -> Vec<(Range<usize>, Range<usize>)> {
    let left_chars: Vec<(usize, char)> = left.char_indices().collect();
    let right_chars: Vec<(usize, char)> = right.char_indices().collect();
    let left_len = left_chars.len();
    let right_len = right_chars.len();
    let mut common = vec![vec![0u32; right_len + 1]; left_len + 1];
    for left_index in (0..left_len).rev() {
        for right_index in (0..right_len).rev() {
            common[left_index][right_index] =
                if left_chars[left_index].1 == right_chars[right_index].1 {
                    common[left_index + 1][right_index + 1] + 1
                } else {
                    common[left_index + 1][right_index].max(common[left_index][right_index + 1])
                };
        }
    }
    let mut ranges = Vec::new();
    let mut left_index = 0;
    let mut right_index = 0;
    while left_index < left_len || right_index < right_len {
        if left_index < left_len
            && right_index < right_len
            && left_chars[left_index].1 == right_chars[right_index].1
            && common[left_index][right_index] == common[left_index + 1][right_index + 1] + 1
        {
            left_index += 1;
            right_index += 1;
            continue;
        }
        let left_start = char_offset(&left_chars, left_index, left.len());
        let right_start = char_offset(&right_chars, right_index, right.len());
        while left_index < left_len || right_index < right_len {
            let equal = left_index < left_len
                && right_index < right_len
                && left_chars[left_index].1 == right_chars[right_index].1
                && common[left_index][right_index] == common[left_index + 1][right_index + 1] + 1;
            if equal {
                break;
            }
            if right_index == right_len
                || (left_index < left_len
                    && common[left_index + 1][right_index] >= common[left_index][right_index + 1])
            {
                left_index += 1;
            } else {
                right_index += 1;
            }
        }
        let left_end = char_offset(&left_chars, left_index, left.len());
        let right_end = char_offset(&right_chars, right_index, right.len());
        if left_start != left_end || right_start != right_end {
            ranges.push((left_start..left_end, right_start..right_end));
        }
    }
    ranges
}

/// The byte offset of one character in a line.
fn char_offset(chars: &[(usize, char)], index: usize, len: usize) -> usize {
    if index < chars.len() {
        chars[index].0
    } else {
        len
    }
}

/// Rewrites code identifiers in one file.
///
/// A file the dialect cannot classify is left unchanged.
fn rewrite_file(path: &Path, from: &str, to: &str) -> Result<(), String> {
    let Some(dialect) = crate::dialect::for_path(&path.to_string_lossy()) else {
        return Ok(());
    };
    let Some(classify) = dialect.spans else {
        return Ok(());
    };
    let original = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let updated = replace_identifiers(&original, &classify(&original), from, to);
    if updated != original {
        fs::write(path, updated)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Replaces identifier spans that hold `from`.
///
/// A keyword, comment, or string keeps its text.
fn replace_identifiers(source: &str, spans: &[Span], from: &str, to: &str) -> String {
    let mut renamed = String::new();
    let mut cursor = 0;
    for span in spans {
        if span.kind == SpanKind::Identifier && &source[span.start..span.end] == from {
            renamed.push_str(&source[cursor..span.start]);
            renamed.push_str(to);
            cursor = span.end;
        }
    }
    renamed.push_str(&source[cursor..]);
    renamed
}

#[cfg(test)]
mod tests {
    use super::{changed_line_pairs, identifier_edits, replace_identifiers, tightest_scope};
    use crate::dialect::{Span, SpanKind, RUST};

    #[test]
    fn pairs_a_changed_line_and_not_an_inserted_comment() {
        let left = "fn fibonacci(n: u32) -> u64 {\n    match n {\n";
        let right = "/// where `fib(0)` is 0.\nfn fib(n: u32) -> u64 {\n    match n {\n";
        let pairs = changed_line_pairs(left, right);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].0.text.contains("fibonacci"));
        assert!(pairs[0].1.text.starts_with("fn fib("));
    }

    #[test]
    fn an_inserted_comment_is_not_an_identifier_change() {
        let left = "fn fibonacci(n: u32) -> u64 {\n    println!(\"{n}: {}\", fibonacci(n));\n";
        let right = "/// where `fib(0)` is 0.\nfn fib(n: u32) -> u64 {\n    println!(\"{n}: {}\", fib(n));\n";
        let classify = RUST.spans.unwrap();
        let edits = identifier_edits(left, right, &classify(left), &classify(right));
        assert_eq!(
            edits,
            vec![
                ("fibonacci".to_string(), "fib".to_string()),
                ("fibonacci".to_string(), "fib".to_string()),
            ]
        );
    }

    #[test]
    fn a_trailing_comment_does_not_hide_the_rename() {
        let left = "fn use_it() { fibonacci(); }\n";
        let right = "fn use_it() { fib(); } // fibonacci\n";
        let classify = RUST.spans.unwrap();
        let edits = identifier_edits(left, right, &classify(left), &classify(right));
        assert_eq!(edits, vec![("fibonacci".to_string(), "fib".to_string())]);
    }

    #[test]
    fn scope_is_the_tightest_path_that_covers_every_use() {
        assert_eq!(tightest_scope(&["src/main.rs"]), "src/main.rs");
        assert_eq!(tightest_scope(&["src/a.rs", "src/b.rs"]), "src");
        assert_eq!(tightest_scope(&["src/foo/a.rs", "src/bar/b.rs"]), "src");
        assert_eq!(tightest_scope(&["a.rs", "nested/b.rs"]), ".");
    }

    #[test]
    fn replaces_identifier_spans_only() {
        let source = "keep NAME tail";
        let spans = [Span {
            start: 5,
            end: 9,
            kind: SpanKind::Identifier,
        }];
        assert_eq!(
            replace_identifiers(source, &spans, "NAME", "X"),
            "keep X tail"
        );
    }
}
