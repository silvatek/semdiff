//! Tests of how apply carries out a file-level change.

use crate::filesystem::Tree;

#[test]
fn rejects_a_file_whose_old_hash_does_not_match() {
    let (tree, document) = changed_note();
    let document = document.replacen(
        "9e9894bfcaf8d63b9898e90db0fe05f5a0388d45bcf60a0a208ad31153072142",
        &"0".repeat(64),
        1,
    );
    let message = tree.apply("edited", &document).unwrap_err();
    assert!(message.contains("do not match the old hash"), "{message}");
    assert_eq!(tree.read("edited/note.txt"), b"old line\n");
}

#[test]
fn rejects_a_file_whose_new_hash_does_not_match() {
    let (tree, document) = changed_note();
    let document = document.replacen(
        "f77de5e87174686ce6fca0e3d03893b38dc6020febaa55578a8b20b289b5c362",
        &"f".repeat(64),
        1,
    );
    let message = tree.apply("edited", &document).unwrap_err();
    assert!(message.contains("do not match the new hash"), "{message}");
    assert_eq!(tree.read("edited/note.txt"), b"old line\n");
}

fn changed_note() -> (Tree, String) {
    let tree = Tree::new();
    tree.write("left/note.txt", "old line\n");
    tree.write("right/note.txt", "He said \"hello\"\nNext line\n");
    tree.write("edited/note.txt", "old line\n");
    let document = tree.compare("left", "right");
    (tree, document)
}
