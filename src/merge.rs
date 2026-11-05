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
fn extend_adjacent(
    merged: &mut Vec<crate::semdif::Change>,
    change: &crate::semdif::Change,
) -> bool {
    match (merged.last_mut(), change) {
        (
            Some(crate::semdif::Change::LineRemoved {
                path: previous_path,
                old_end: previous_end,
                new: previous_new,
                text: previous_text,
                ..
            }),
            crate::semdif::Change::LineRemoved {
                path,
                old,
                old_end,
                new,
                text,
            },
        ) => {
            if previous_path != path || *previous_end + 1 != *old || previous_new != new {
                return false;
            }
            *previous_end = *old_end;
            append_line(previous_text, text);
            true
        }
        (
            Some(crate::semdif::Change::LineAdded {
                path: previous_path,
                old: previous_old,
                new: previous_new,
                text: previous_text,
            }),
            crate::semdif::Change::LineAdded {
                path,
                old,
                new,
                text,
            },
        ) => {
            let Some(next_new) = previous_new.checked_add(line_count(previous_text)) else {
                return false;
            };
            if previous_path != path || previous_old != old || next_new != *new {
                return false;
            }
            append_line(previous_text, text);
            true
        }
        (
            Some(crate::semdif::Change::LineModified {
                path: previous_path,
                old: previous_old,
                new: previous_new,
                text: previous_text,
            }),
            crate::semdif::Change::LineModified {
                path,
                old,
                new,
                text,
            },
        ) => {
            let count = line_count(previous_text);
            let Some(next_old) = previous_old.checked_add(count) else {
                return false;
            };
            let Some(next_new) = previous_new.checked_add(count) else {
                return false;
            };
            if previous_path != path || next_old != *old || next_new != *new {
                return false;
            }
            append_line(previous_text, text);
            true
        }
        _ => false,
    }
}
