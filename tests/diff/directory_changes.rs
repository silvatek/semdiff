//! Tests of identifying and classifying changes at the directory level.

use crate::filesystem::Tree;

#[test]
fn records_empty_directories_added_and_removed() {
    let tree = Tree::with_ignores(["target"]);
    tree.dir("left/gone/nested");
    tree.dir("left/target/skipped");
    tree.dir("right/vacant/nested");
    tree.dir("edited/gone/nested");
    tree.dir("edited/target/skipped");
    tree.write("left/keep.txt", "same\n");
    tree.write("right/keep.txt", "same\n");
    tree.write("edited/keep.txt", "same\n");

    let document = tree.compare("left", "right");

    assert_eq!(
        document.split_once("changes:\n").expect("changes list").1,
        "  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"gone/nested\"\n  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"gone\"\n  - scope: \".\"\n    type: directory-added\n    details:\n      path: \"vacant\"\n  - scope: \".\"\n    type: directory-added\n    details:\n      path: \"vacant/nested\"\n"
    );
    assert!(!document.contains("target"));

    tree.apply("edited", &document).unwrap();
    assert!(!tree.exists("edited/gone"));
    assert!(tree.is_dir("edited/target/skipped"));
    assert!(tree.is_dir("edited/vacant/nested"));
    assert_eq!(tree.read("edited/keep.txt"), b"same\n");
}

#[test]
fn ignores_the_target_path_and_keeps_a_nested_directory_named_target() {
    let tree = Tree::with_ignores(["target"]);
    tree.write("left/target/skip.txt", "old\n");
    tree.write("right/target/skip.txt", "new\n");
    tree.write("left/src/target/lib.rs", "old\n");
    tree.write("right/src/target/lib.rs", "new\n");
    tree.write("left/targeting/note.txt", "old\n");
    tree.write("right/targeting/note.txt", "new\n");
    tree.write("left/keep.txt", "same\n");
    tree.write("right/keep.txt", "same\n");

    let document = tree.compare("left", "right");

    assert!(!document.contains("target/skip.txt"), "{document}");
    assert!(document.contains("src/target/lib.rs"), "{document}");
    assert!(document.contains("targeting/note.txt"), "{document}");

    let open = Tree::new();
    open.write("left/target/skip.txt", "old\n");
    open.write("right/target/skip.txt", "new\n");
    let document = open.compare("left", "right");
    assert!(document.contains("target/skip.txt"), "{document}");
}

#[test]
fn replaces_a_directory_with_a_different_file() {
    let tree = Tree::new();
    tree.dir("left/notes/sub");
    tree.dir("edited/notes/sub");
    tree.dir("right");
    tree.write("left/notes/a.txt", "inside\n");
    tree.write("left/notes/sub/b.txt", "other\n");
    tree.write("edited/notes/a.txt", "inside\n");
    tree.write("edited/notes/sub/b.txt", "other\n");
    tree.write("right/notes", "replaced\n");

    let document = tree.compare("left", "right");

    assert_eq!(
        document.split_once("changes:\n").expect("changes list").1,
        "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"notes/a.txt\"\n      hash: \"7b2441693c861bf6969869d8b6f45f098bc8ef07b78ca043a1cb663159aabb10\"\n  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"notes/sub/b.txt\"\n      hash: \"7e4fa2eb8c7ac089739d5defc4489fad68a100d92082ca35c6b40a4524821f87\"\n  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"notes/sub\"\n  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"notes\"\n  - scope: \".\"\n    type: file-added\n    details:\n      path: \"notes\"\n      hash: \"e2208f01e42b2cab0fef975b55dc70d39579dd3d0c5d0758c499baa5109ef187\"\n      contents: \"replaced\\n\"\n"
    );

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/notes"), b"replaced\n");
    assert!(!tree.exists("edited/notes/a.txt"));
    assert!(!tree.exists("edited/notes/sub"));
}

#[test]
fn removes_a_directory_left_empty_by_a_rename() {
    let tree = Tree::new();
    tree.dir("left/old");
    tree.dir("right/new");
    tree.dir("edited/old");
    let note = "moved\n";
    tree.write("left/old/note.txt", note);
    tree.write("right/new/note.txt", note);
    tree.write("edited/old/note.txt", note);

    let document = tree.compare("left", "right");

    assert!(document.contains(
        "    type: file-renamed\n    details:\n      from: \"old/note.txt\"\n      to: \"new/note.txt\"\n      hash: \"2c85ab0700b597297552509665d1f5a95111d16c5416fdc88d5bb85fcf4d0017\"\n"
    ));
    assert!(document.contains("    type: directory-deleted\n    details:\n      path: \"old\"\n"));
    assert!(!document.contains("directory-added"));

    tree.apply("edited", &document).unwrap();
    assert!(!tree.exists("edited/old"));
    assert_eq!(tree.read("edited/new/note.txt"), note.as_bytes());
}
