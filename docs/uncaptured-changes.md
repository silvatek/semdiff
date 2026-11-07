# Uncaptured changes

Decisions from a review of gaps in the current semdiff comparison. Each gap is either skipped or coarser.

Skipped means apply does not reproduce the right-hand tree. Coarser means the tree can be reproduced, but not as the edit that happened.

| # | Change | Kind | Status |
| --- | --- | --- | --- |
| 1 | Empty directories | Skipped | Fixed |
| 2 | Directory replaced by a different file | Skipped | Fixed |
| 3 | Directory replaced by a renamed inner file | Skipped | Fix later |
| 4 | Permissions, symlinks, and special files | Skipped | Leave |
| 5 | Files under `target` | Skipped | Fixed |
| 6 | Rename with edited contents | Coarser | Leave |
| 7 | Identical copies | Coarser | Leave |
| 8 | Directory rename | Coarser | Leave |
| 9 | Edits inside a changed file | Coarser | Fix |
| 10 | File moved under the same name | Coarser | Fix later |

**Fixed** means the change is in the code. **Fix** means we will change semdiff. **Fix later** means the change is deferred. **Leave** means the current behavior stays.

## 1. Empty directories

Fixed. An empty directory that appears or disappears is recorded as `directory-added` or `directory-deleted`. A directory left empty because its files moved or were deleted is removed too. Nested removals come before their parents, and additions come parents first. Apply replays that order.

## 2. Directory replaced by a different file

Fixed. A directory replaced by a file whose bytes do not match any file inside it is recorded as the inner deletions, the directory removal, and the new file. Those records are ordered so the files inside go first, then the directory, then the new file. Apply can write the file because the path is no longer a directory.

## 3. Directory replaced by a renamed inner file

Fix later. A directory replaced by a file whose bytes match one file inside it is recorded as a rename onto the directory path, then a removal of the directory.

That rename is not a legal filesystem operation. A path cannot be the directory and the file at once, and `rename` will not replace a directory with a file. While the inner file is still inside, the directory is not empty, so apply stops with `Directory not empty (os error 39)` and does not apply later records. The tree is left unchanged. Reversing the two records fails as well, because the directory still holds the file.

Reaching that end state in one working tree takes an indirect sequence: copy the bytes out, remove the directory, then write the file. Two snapshots can still differ this way. The ordinary version is item 2, where the new file's bytes differ. The content match is what makes semdiff call this one a rename.

The reachable recording is the one item 2 already uses: delete the inner file, delete the directory, then add the file with those bytes. The bytes leave the tree in the semdif, so apply does not need a temporary path. That change is deferred.

## 4. Permissions, symlinks, and special files

Leave. Comparison follows symlinks and reads file bytes only, so mode bits, symlinks as links, dangling symlinks, and special files such as fifos are not recorded. Apply writes a regular file and does not recreate them. Support may be added later.

## 5. Files under `target`

Fixed. A directory named `target` is compared like any other directory. The earlier skip of Cargo build output is removed in pull request #26.

## 6. Rename with edited contents

Leave. A renamed file whose bytes also changed stays a `file-deleted` plus a `file-added` carrying the new bytes. Apply rebuilds the tree correctly. Recording that the two paths are the same file can wait. It is low priority.

## 7. Identical copies

Leave. A rename is recorded only when one unpaired file on the left and one on the right share a hash. When two or more unpaired files share a hash, none of them is called a rename. The left-hand paths stay `file-deleted` and the right-hand paths stay `file-added`. Apply rebuilds the tree correctly. Guessing which copy moved can wait.

## 8. Directory rename

Leave. There is no `directory-renamed` record. Renaming a directory of unchanged files stays one `file-renamed` per file. Apply creates the new directory while moving those files, then deletes the old directory once it is empty. A renamed directory that still holds files is not also `directory-added`. An empty directory rename stays `directory-deleted` plus `directory-added`, because nothing identifies the two paths as the same directory. Apply rebuilds the tree correctly. Recording that the directory itself moved can wait.

## 9. Edits inside a changed file

Fix. A file present at the same path on both sides is compared byte for byte. Matching copies are left out. Any difference is one `file-changed` record holding the entire new file, as a YAML string or base64. Apply replaces the whole file with those bytes.

That whole-file record is the starting point for the semantic difference engine. The Fibonacci sample is this case: the added line `// Print Fibonacci numbers.` in `src/main.rs` is recorded as the whole new file. Later changes will describe the edit itself, such as a line comment, a line edit, or a symbol rename, instead of only the new bytes.

## 10. File moved under the same name

Fix later. This narrows item 6. A file that moves and also changes stays `file-deleted` plus `file-added`, because the hashes differ. The filename is the final component of the path. When exactly one deleted file and exactly one added file share that name, the two copies can be compared. If the contents are broadly similar, the pair is one `file-moved` record instead of the deletion and the addition. The record names both paths and carries the new bytes, so apply removes the old file and writes the new one. Several files with that name, or contents that are not similar, stay a deletion and an addition.
