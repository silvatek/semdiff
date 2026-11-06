//! The text printed for `--help`, and the short usage printed for bad arguments.
//!
//! The full help begins with [`USAGE`], so the two messages stay the same text.
//! The rest of the help is chosen from [`details_by_language`] by language code.
//! English (`en`) is the default. Another language is another entry in that map.

use std::collections::BTreeMap;

/// The short message printed when the arguments are wrong.
pub(crate) const USAGE: &str = "\
Compare two sources and write a semdif. --apply replays a semdif onto a directory.
A semdif is a list of differences, with semantic context where available.

Usage: semdiff [--ignore=<file>] <left> <right> <output>
       semdiff --apply [--ignore=<file>] [--ignore-hash] <semdif> <directory>
       semdiff --help

An output path of - writes the semdif to standard output, and the listing to standard error.
A semdif path of - reads the semdif from standard input.
--help writes a full description to standard output.
";

/// The language code used when none is selected.
///
/// English is the only language so far.
const DEFAULT_LANGUAGE: &str = "en";

/// The rest of the English help, after [`USAGE`].
///
/// It describes each argument, what a comparison records, what apply checks,
/// and the limitations that stay.
const DETAILS_EN: &str = "\
Arguments

  --help
      Prints this message on standard output and exits. The other arguments
      are left unused.

  left, right
      The two sources of a comparison. Both are files, or both are directories.
      A missing path is reported as \"not a file or directory\". A file paired
      with a directory is reported as \"sources must both be files or both be
      directories\".

  output
      Where the semdif is written. A path of - writes the semdif to standard
      output and writes the listing to standard error. Any other path is a
      file, and the listing goes to standard output. The listing is the tool
      version, the line \"Comparing <left> and <right>\", and each path present
      in both sources.

  --apply
      Replays a semdif onto a directory. The flag may appear anywhere among
      the apply arguments. A second --apply is a usage error.

  semdif
      The semdif to apply. A path of - reads it from standard input, and that
      path need not exist. Any other path must name a file. A missing file is
      reported as \"not a file\".

  directory
      The directory --apply edits. It must be a directory. A file is reported
      as \"not a directory\". A rename moves a file inside this directory.

  --ignore=<file>
      Paths the walk leaves out, one relative path per line. The flag uses an
      equals sign and may appear anywhere among the other arguments. One file
      is accepted. An empty name, a second file, or the form without an equals
      sign is a usage error. A missing file is reported as \"not a file\".

      Each line is trimmed. A blank line is skipped. A backslash is read as a
      slash. An entry covers that path and every path under it. An entry of
      target leaves src/target and targeting in the walk. There is no default
      list. The same file is used for a comparison and for --apply. The walk
      is what a comparison collects, and what a directory-scoped apply
      collects. A change that names a path is still applied.

  --ignore-hash
      Applies a file-added, file-deleted, or file-renamed change when its
      recorded SHA-256 does not match. The flag is valid only with --apply,
      and it may appear anywhere among those arguments. A second copy is a
      usage error. A file-changed record still checks its old and new hashes.
      A missing hash is still rejected, as \"file-deleted is missing hash\"
      and the same form for the other kinds. A hash that is not 64 hexadecimal
      digits is rejected as \"file-deleted has invalid hash\".

      Any other argument that starts with - is a usage error. A lone - is a
      path, the standard stream.

Exit status

  0  The command finished, or --help was printed.
  1  A path is missing, the sources differ in kind, or a read, write, or apply
     check failed.
  2  The arguments are wrong. The short usage is written to standard error.

What a comparison records

  Files present at the same path on both sides are paired. Matching bytes are
  left out of the semdif. The listing still names each paired path.

  A changed text file is recorded as line-added, line-removed, and
  line-modified edits. The file-changed record keeps the old and new SHA-256
  of the whole file. Adjacent edits of the same kind and scope become one
  record when they share the same comment flag and the same whitespace flag.
  A full-line comment is marked isComment. Insignificant whitespace is marked
  isWhitespace. False is left out.

  Bytes that are not UTF-8 stay one whole-file replacement. The record stores
  them as base64 and names that encoding.

  An added or removed run that is one complete named block is recorded as
  named-block-added or named-block-removed, with a category and a name.
  A method's scope names the file and the enclosing block, as file[Name].

  A line edit that changes exactly one identifier, when that old name occurs
  once on the line, is identifier-renamed. The same rename joins across files
  when the directory that covers them has no other occurrence of that name.
  A file with no dialect stays a line modification.

  A file only on the left is file-deleted. A file only on the right is
  file-added, and the semdif keeps its bytes. One unpaired file on each side
  with the same SHA-256 is file-renamed. Each of those records includes the
  hash of the bytes. An empty directory that appears or disappears is
  directory-added or directory-deleted, including a directory left empty
  because its files moved. Nested removals come before their parents, and
  additions come parents first.

  A directory replaced by a file whose bytes match none of the files inside
  it is the inner deletions, the directory removal, and the new file.

  The header records the tool version, the UTC time as YYYY-MM-DDTHH:MM:SSZ,
  hash sha256, text-encoding utf-8, and binary-encoding base64. Apply replays
  the changes.

Dialects

  Rust (.rs)
  Python (.py)

  The language documentation describes how a dialect reads comments, blocks,
  and names. A file with another extension has no dialect. These files are
  treated as plain text, not ignored.

What apply checks

  Apply replays the changes in order. A file-changed record checks the old
  hash against the file as it is, rebuilds the file, and checks the new hash
  against the bytes about to be written. A mismatch leaves that file
  unchanged and is reported as \"cannot apply PATH: contents do not match the
  old hash\" or \"the new hash\". --ignore-hash leaves those checks in place.

  A file-deleted or file-renamed change checks the hash of the file in the
  tree. A file-added change checks the hash of the bytes about to be written.
  A mismatch is reported as \"cannot apply PATH: contents do not match the
  hash\" and leaves the tree unchanged. --ignore-hash applies those three
  changes anyway.

Limitations

  Comparison follows symlinks and reads file bytes. Mode bits, a symlink as a
  link, a dangling symlink, and a special file such as a fifo stay
  unrecorded. Apply writes a regular file.

  A renamed file whose bytes also changed is file-deleted plus file-added.
  The addition carries the new bytes.

  A rename is recorded when exactly one unpaired file on each side shares a
  hash. When two or more unpaired files share a hash, the left-hand paths are
  file-deleted and the right-hand paths are file-added.

  A directory of unchanged files that moves is one file-renamed per file.
  Apply creates the new directory while moving those files, then deletes the
  old directory once it is empty. An empty directory that moves is
  directory-deleted plus directory-added.

  A directory replaced by a file whose bytes match one file inside it is
  recorded as a rename onto the directory path, then a removal of the
  directory. Apply stops with \"Directory not empty (os error 39)\" and leaves
  the tree unchanged.

  The ignore file is one list of paths. It has no globs and no comment syntax.

  A brace written inside a string counts as a block boundary. An identifier
  rename reads comments and strings as ordinary text, so a one-name edit
  there is still a rename.

  A Rust attribute, pub(in path), or impl<T> leaves the run a line edit. A
  Python decorator above def or class, or an async def, leaves the run a
  line edit.
";

/// The detailed help for each language code.
///
/// English is the only entry. Another language is another code and another text.
fn details_by_language() -> BTreeMap<&'static str, &'static str> {
    let mut texts = BTreeMap::new();
    texts.insert("en", DETAILS_EN);
    texts
}

/// The detailed help for `language`.
///
/// An unknown code uses [`DEFAULT_LANGUAGE`].
fn details_for(language: &str) -> &'static str {
    let texts = details_by_language();
    texts
        .get(language)
        .or_else(|| texts.get(DEFAULT_LANGUAGE))
        .copied()
        .expect("English help is present")
}

/// The full help, beginning with [`USAGE`].
///
/// The description after the usage is the [`DEFAULT_LANGUAGE`] entry.
pub(crate) fn help_text() -> String {
    let details = details_for(DEFAULT_LANGUAGE);
    let mut text = String::with_capacity(USAGE.len() + details.len());
    text.push_str(USAGE);
    text.push_str(details);
    text
}

/// Writes [`help_text`] to standard output.
pub(crate) fn print_help() {
    print!("{}", help_text());
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use super::{details_by_language, details_for, help_text, DEFAULT_LANGUAGE, DETAILS_EN, USAGE};

    #[test]
    fn help_starts_with_the_usage_text() {
        let help = help_text();
        assert!(help.starts_with(USAGE));
        assert!(help.contains(DETAILS_EN));
        assert!(help.contains("--help writes a full description"));
        assert!(help.contains("\nArguments\n"));
        assert!(help.contains("\nWhat a comparison records\n"));
        assert!(help.contains("\nDialects\n"));
        assert!(help.contains("Rust (.rs)"));
        assert!(help.contains("Python (.py)"));
        assert!(help.contains("treated as plain text, not ignored"));
        assert!(help.contains("\nWhat apply checks\n"));
        assert!(help.contains("\nLimitations\n"));
        assert!(help.ends_with('\n'));
    }

    #[test]
    fn english_is_the_default_language() {
        let texts = details_by_language();
        assert_eq!(texts.get(DEFAULT_LANGUAGE), Some(&DETAILS_EN));
        assert_eq!(details_for("en"), DETAILS_EN);
        assert_eq!(details_for("missing"), DETAILS_EN);
    }
}
