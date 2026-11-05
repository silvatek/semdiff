use super::run;

use std::fs;

#[test]
fn records_line_changes_for_a_pair_whose_contents_differ() {
    let root = std::env::temp_dir().join(format!("semdiff-content-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::create_dir_all(&edited).unwrap();
    let same = "unchanged\n";
    let before = "old line\n";
    let after = "He said \"hello\"\nNext line\n";
    let removed = "keep\ngone\n";
    let kept = "keep\n";
    let binary = b"\x00\xff";
    fs::write(left.join("same.txt"), same).unwrap();
    fs::write(right.join("same.txt"), same).unwrap();
    fs::write(edited.join("same.txt"), same).unwrap();
    fs::write(left.join("note.txt"), before).unwrap();
    fs::write(right.join("note.txt"), after).unwrap();
    fs::write(edited.join("note.txt"), before).unwrap();
    fs::write(left.join("drop.txt"), removed).unwrap();
    fs::write(right.join("drop.txt"), kept).unwrap();
    fs::write(edited.join("drop.txt"), removed).unwrap();
    fs::write(left.join("data.bin"), b"old").unwrap();
    fs::write(right.join("data.bin"), binary).unwrap();
    fs::write(edited.join("data.bin"), b"old").unwrap();

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
    assert!(!document.contains("same.txt"), "{document}");
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"drop.txt\"\n      old: 2\n      new: 2\n      text: \"gone\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-modified\n    details:\n      path: \"note.txt\"\n      old: 1\n      new: 1\n      text: \"He said \\\"hello\\\"\"\n  - scope: \".\"\n    type: line-added\n    details:\n      path: \"note.txt\"\n      old: 2\n      new: 2\n      text: \"Next line\"\n"
    ));
    assert!(document.contains("path: \"data.bin\""));
    assert!(document.contains("encoding: \"base64\""));
    assert!(document.contains("type: file-changed"));
    assert!(!document.as_bytes().contains(&0xff));

    let applied = run(&["--apply", output_arg, edited_arg]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(fs::read(edited.join("same.txt")).unwrap(), same.as_bytes());
    assert_eq!(fs::read(edited.join("note.txt")).unwrap(), after.as_bytes());
    assert_eq!(fs::read(edited.join("drop.txt")).unwrap(), kept.as_bytes());
    assert_eq!(fs::read(edited.join("data.bin")).unwrap(), binary);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn replays_a_modification_a_removal_and_an_addition() {
    let root = std::env::temp_dir().join(format!("semdiff-lines-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::create_dir_all(&edited).unwrap();
    let before = "same\nold\ngone\ntail\n";
    let after = "same\nnew\ntail\nextra\n";
    fs::write(left.join("edit.txt"), before).unwrap();
    fs::write(right.join("edit.txt"), after).unwrap();
    fs::write(edited.join("edit.txt"), before).unwrap();

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
    assert_eq!(
        document.split_once("changes:\n").expect("changes list").1,
        "  - scope: \".\"\n    type: line-modified\n    details:\n      path: \"edit.txt\"\n      old: 2\n      new: 2\n      text: \"new\"\n  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"edit.txt\"\n      old: 3\n      new: 3\n      text: \"gone\"\n  - scope: \".\"\n    type: line-added\n    details:\n      path: \"edit.txt\"\n      old: 5\n      new: 4\n      text: \"extra\"\n"
    );

    let applied = run(&["--apply", output_arg, edited_arg]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(fs::read(edited.join("edit.txt")).unwrap(), after.as_bytes());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn records_a_whole_file_when_only_the_final_newline_differs() {
    let root = std::env::temp_dir().join(format!("semdiff-newline-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::create_dir_all(&edited).unwrap();
    fs::write(left.join("note.txt"), "hello").unwrap();
    fs::write(right.join("note.txt"), "hello\n").unwrap();
    fs::write(edited.join("note.txt"), "hello").unwrap();

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
    assert!(document.contains(
        "  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"note.txt\"\n      contents: \"hello\\n\"\n"
    ));

    let applied = run(&["--apply", output_arg, edited_arg]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(fs::read(edited.join("note.txt")).unwrap(), b"hello\n");

    let _ = fs::remove_dir_all(root);
}
