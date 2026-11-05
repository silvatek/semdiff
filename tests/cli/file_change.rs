use super::tree::Tree;

#[test]
fn writes_a_file_renamed_change() {
    let tree = Tree::new();
    let quote = "Brevity is the soul of wit.\n";
    tree.write("left/quote.txt", quote);
    tree.write("right/quotation.txt", quote);
    tree.write("left/notes.txt", "left only\n");
    tree.write("right/other.txt", "right only\n");

    let document = tree.compare("left", "right");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"notes.txt\"\n  - scope: \".\"\n    type: file-added\n    details:\n      path: \"other.txt\"\n      contents: \"right only\\n\"\n  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );
}

#[test]
fn hashes_past_the_first_buffer() {
    let tree = Tree::new();
    let mut same = vec![1_u8; 9_000];
    same[8_500] = 7;
    let shared_prefix = vec![2_u8; 9_000];
    let mut shared_prefix_changed = shared_prefix.clone();
    shared_prefix_changed[8_500] = 4;
    tree.write("left/same.bin", &same);
    tree.write("right/same-renamed.bin", &same);
    tree.write("left/left.bin", &shared_prefix);
    tree.write("right/right.bin", &shared_prefix_changed);

    let document = tree.compare("left", "right");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert!(changes.contains(
        "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"left.bin\"\n"
    ));
    assert!(changes.contains(
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"same.bin\"\n      to: \"same-renamed.bin\"\n"
    ));
    let contents_line = document
        .lines()
        .find(|line| line.contains("\\u0002"))
        .expect("added file contents");
    assert!(contents_line.contains("\\u0002"));
    assert!(!contents_line
        .chars()
        .any(|character| (character as u32) < 0x20));
}

#[test]
fn adds_a_file_and_restores_contents_that_yaml_cannot_hold_literally() {
    let tree = Tree::new();
    let kept = "same\n";
    let tricky = "He said \"hello\"\tand wrote C:\\temp.\r\nNext: line\n  spaced\0\u{2028}";
    let binary = b"\x00\xff\"\\\n: ";
    tree.write("left/keep.txt", kept);
    tree.write("right/keep.txt", kept);
    tree.write("edited/keep.txt", kept);
    tree.write("right/tricky.txt", tricky);
    tree.write("right/empty.txt", "");
    tree.write("right/nested/data.bin", binary);

    let document = tree.compare("left", "right");

    let tricky_line = document
        .lines()
        .find(|line| line.contains("hello"))
        .expect("tricky contents");
    assert!(tricky_line.contains("\\\""));
    assert!(tricky_line.contains("\\\\"));
    assert!(tricky_line.contains("\\n"));
    assert!(tricky_line.contains("\\r"));
    assert!(tricky_line.contains("\\t"));
    assert!(tricky_line.contains("\\u0000"));
    assert!(tricky_line.contains("\\u2028"));
    assert!(!tricky_line.chars().any(|character| {
        character == '\n'
            || character == '\r'
            || character == '\t'
            || character == '\0'
            || character == '\u{2028}'
    }));
    assert!(document.contains("path: \"empty.txt\""));
    assert!(document.contains("contents: \"\""));
    assert!(document.contains("encoding: \"base64\""));
    assert!(document.contains("path: \"nested/data.bin\""));
    assert!(!document.as_bytes().contains(&0xff));

    tree.apply("edited", &document).unwrap();
    assert_eq!(tree.read("edited/keep.txt"), kept.as_bytes());
    assert_eq!(tree.read("edited/tricky.txt"), tricky.as_bytes());
    assert_eq!(tree.read("edited/empty.txt"), b"");
    assert_eq!(tree.read("edited/nested/data.bin"), binary);
}

#[test]
fn detects_a_rename_between_individual_files() {
    let tree = Tree::new();
    let quote = "Brevity is the soul of wit.\n";
    tree.write("left/quote.txt", quote);
    tree.write("right/quotation.txt", quote);

    let document = tree.compare("left/quote.txt", "right/quotation.txt");

    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );
}
