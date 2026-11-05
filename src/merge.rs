/// Collapses adjacent changes before they are written to the semdif file.
///
/// A run of line edits in one file becomes one record. Removals follow one
/// another on the old side and share the new line that follows them, so
/// `lines` is `-1` or `-2`. Additions share an insertion point and follow one
/// another on the new side, so `lines` is `+1` or `+2`. Modifications follow
/// one another on both sides. They do not change the line count, so `lines`
/// stays `+0`. The merged record keeps the first line number. Its text keeps
/// every line, joined by a newline. An identifier rename stays its own record.
pub(crate) fn merge_adjacent(changes: Vec<crate::semdif::Change>) -> Vec<crate::semdif::Change> {
    let mut merged = Vec::new();
    for change in changes {
        if !extend_adjacent(&mut merged, &change) {
            merged.push(change);
        }
    }
    merged
}

fn line_count(text: &str) -> usize {
    text.split('\n').count()
}

fn append_line(previous: &mut String, text: &str) {
    previous.push('\n');
    previous.push_str(text);
}

/// Joins `change` onto the previous record when the two are one adjacent run.
///
/// Records of different kinds, or in different files, stay separate. Each kind
/// then checks whether its line numbers continue.
fn extend_adjacent(
    merged: &mut Vec<crate::semdif::Change>,
    change: &crate::semdif::Change,
) -> bool {
    if !same_line_scope(merged.last(), change) {
        return false;
    }
    match (merged.last_mut(), change) {
        (
            Some(crate::semdif::Change::LineRemoved {
                old_end: previous_end,
                new: previous_new,
                text: previous_text,
                ..
            }),
            crate::semdif::Change::LineRemoved {
                old,
                old_end,
                new,
                text,
                ..
            },
        ) => {
            if *previous_end + 1 != *old || previous_new != new {
                return false;
            }
            *previous_end = *old_end;
            append_line(previous_text, text);
            true
        }
        (
            Some(crate::semdif::Change::LineAdded {
                old: previous_old,
                new: previous_new,
                text: previous_text,
                ..
            }),
            crate::semdif::Change::LineAdded { old, new, text, .. },
        ) => {
            let Some(next_new) = previous_new.checked_add(line_count(previous_text)) else {
                return false;
            };
            if previous_old != old || next_new != *new {
                return false;
            }
            append_line(previous_text, text);
            true
        }
        (
            Some(crate::semdif::Change::LineModified {
                old: previous_old,
                new: previous_new,
                text: previous_text,
                ..
            }),
            crate::semdif::Change::LineModified { old, new, text, .. },
        ) => {
            let count = line_count(previous_text);
            let Some(next_old) = previous_old.checked_add(count) else {
                return false;
            };
            let Some(next_new) = previous_new.checked_add(count) else {
                return false;
            };
            if next_old != *old || next_new != *new {
                return false;
            }
            append_line(previous_text, text);
            true
        }
        _ => false,
    }
}

/// Two line edits can continue one record when they have the same kind and file.
fn same_line_scope(
    previous: Option<&crate::semdif::Change>,
    change: &crate::semdif::Change,
) -> bool {
    match (previous, change) {
        (
            Some(crate::semdif::Change::LineRemoved {
                path: previous_path,
                ..
            }),
            crate::semdif::Change::LineRemoved { path, .. },
        )
        | (
            Some(crate::semdif::Change::LineAdded {
                path: previous_path,
                ..
            }),
            crate::semdif::Change::LineAdded { path, .. },
        )
        | (
            Some(crate::semdif::Change::LineModified {
                path: previous_path,
                ..
            }),
            crate::semdif::Change::LineModified { path, .. },
        ) => previous_path == path,
        _ => false,
    }
}
