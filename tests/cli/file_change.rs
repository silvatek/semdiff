use super::run;

use std::fs;

#[test]
fn writes_a_file_renamed_change() {
    let root = std::env::temp_dir().join(format!("semdiff-rename-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    fs::write(left.join("quote.txt"), quote).unwrap();
    fs::write(right.join("quotation.txt"), quote).unwrap();
    fs::write(left.join("notes.txt"), "left only\n").unwrap();
    fs::write(right.join("other.txt"), "right only\n").unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-deleted\n    details:\n      path: \"notes.txt\"\n  - scope: \".\"\n    type: file-added\n    details:\n      path: \"other.txt\"\n      contents: \"right only\\n\"\n  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn hashes_past_the_first_buffer() {
    let root = std::env::temp_dir().join(format!("semdiff-hash-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let mut same = vec![1_u8; 9_000];
    same[8_500] = 7;
    let shared_prefix = vec![2_u8; 9_000];
    let mut shared_prefix_changed = shared_prefix.clone();
    shared_prefix_changed[8_500] = 4;
    fs::write(left.join("same.bin"), &same).unwrap();
    fs::write(right.join("same-renamed.bin"), &same).unwrap();
    fs::write(left.join("left.bin"), &shared_prefix).unwrap();
    fs::write(right.join("right.bin"), &shared_prefix_changed).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
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

    let _ = fs::remove_dir_all(root);
}

#[test]
fn adds_a_file_and_restores_contents_that_yaml_cannot_hold_literally() {
    let root = std::env::temp_dir().join(format!("semdiff-added-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(right.join("nested")).unwrap();
    fs::create_dir_all(&edited).unwrap();
    let kept = "same\n";
    let tricky = "He said \"hello\"\tand wrote C:\\temp.\r\nNext: line\n  spaced\0\u{2028}";
    let binary = b"\x00\xff\"\\\n: ";
    fs::write(left.join("keep.txt"), kept).unwrap();
    fs::write(right.join("keep.txt"), kept).unwrap();
    fs::write(edited.join("keep.txt"), kept).unwrap();
    fs::write(right.join("tricky.txt"), tricky).unwrap();
    fs::write(right.join("empty.txt"), "").unwrap();
    fs::write(right.join("nested/data.bin"), binary).unwrap();

    let output_path = root.join("out.yml");
    let left_arg = left.to_str().unwrap();
    let right_arg = right.to_str().unwrap();
    let edited_arg = edited.to_str().unwrap();
    let output_arg = output_path.to_str().unwrap();
    let compared = run(&[left_arg, right_arg, output_arg]);
    let document = fs::read_to_string(&output_path).expect("failed to read semdiff file");

    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
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

    let applied = run(&["--apply", output_arg, edited_arg]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(fs::read_to_string(edited.join("keep.txt")).unwrap(), kept);
    assert_eq!(
        fs::read(edited.join("tricky.txt")).unwrap(),
        tricky.as_bytes()
    );
    assert_eq!(fs::read(edited.join("empty.txt")).unwrap(), b"");
    assert_eq!(fs::read(edited.join("nested/data.bin")).unwrap(), binary);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn detects_a_rename_between_individual_files() {
    let root = std::env::temp_dir().join(format!("semdiff-file-rename-{}", std::process::id()));
    let left_dir = root.join("left");
    let right_dir = root.join("right");
    fs::create_dir_all(&left_dir).unwrap();
    fs::create_dir_all(&right_dir).unwrap();
    let quote = "Brevity is the soul of wit.\n";
    fs::write(left_dir.join("quote.txt"), quote).unwrap();
    fs::write(right_dir.join("quotation.txt"), quote).unwrap();

    let output_path = root.join("out.yml");
    let left = left_dir.join("quote.txt");
    let right = right_dir.join("quotation.txt");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    let changes = document.split_once("changes:\n").expect("changes list").1;
    assert_eq!(
        changes,
        "  - scope: \".\"\n    type: file-renamed\n    details:\n      from: \"quote.txt\"\n      to: \"quotation.txt\"\n"
    );

    let _ = fs::remove_dir_all(root);
}
