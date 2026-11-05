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
    let mut merged: Vec<crate::semdif::Change> = Vec::new();
    for change in changes {
        let extended = match merged.last_mut() {
            Some(previous) => previous.extend(change.as_ref()),
            None => false,
        };
        if !extended {
            merged.push(change);
        }
    }
    merged
}
