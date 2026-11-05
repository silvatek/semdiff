/// Collapses adjacent changes before they are written to the semdif file.
///
/// A run of line-removed records is one removal when the records are for one
/// file, their old lines follow one another, and the same new line follows
/// them. The merged record keeps the first old line. `lines` is the negative
/// count, `-1` for one line and `-2` for two. Other kinds of change stay as
/// they are.
pub(crate) fn merge_adjacent(changes: Vec<crate::semdif::Change>) -> Vec<crate::semdif::Change> {
    let mut merged = Vec::new();
    for change in changes {
        if extend_removed(&mut merged, &change) {
            continue;
        }
        merged.push(change);
    }
    merged
}

/// Adds `change` to the previous line removal when the two are one run.
///
/// The removed text keeps every line, joined by a newline.
fn extend_removed(merged: &mut Vec<crate::semdif::Change>, change: &crate::semdif::Change) -> bool {
    let crate::semdif::Change::LineRemoved {
        path,
        old,
        old_end,
        new,
        text,
    } = change
    else {
        return false;
    };
    let Some(crate::semdif::Change::LineRemoved {
        path: previous_path,
        old_end: previous_end,
        new: previous_new,
        text: previous_text,
        ..
    }) = merged.last_mut()
    else {
        return false;
    };
    if previous_path != path || *previous_end + 1 != *old || previous_new != new {
        return false;
    }
    *previous_end = *old_end;
    previous_text.push('\n');
    previous_text.push_str(text);
    true
}
