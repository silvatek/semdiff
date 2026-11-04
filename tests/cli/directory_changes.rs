use super::run;

use std::fs;
use std::path::Path;

#[test]
fn records_empty_directories_added_and_removed() {
    let root = std::env::temp_dir().join(format!("semdiff-dirs-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(left.join("gone/nested")).unwrap();
    fs::create_dir_all(left.join("target/skipped")).unwrap();
    fs::create_dir_all(right.join("vacant/nested")).unwrap();
    fs::create_dir_all(edited.join("gone/nested")).unwrap();
    fs::create_dir_all(edited.join("target/skipped")).unwrap();
    fs::write(left.join("keep.txt"), "same\n").unwrap();
    fs::write(right.join("keep.txt"), "same\n").unwrap();
    fs::write(edited.join("keep.txt"), "same\n").unwrap();

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
        "  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"gone/nested\"\n  - scope: \".\"\n    type: directory-deleted\n    details:\n      path: \"gone\"\n  - scope: \".\"\n    type: directory-added\n    details:\n      path: \"vacant\"\n  - scope: \".\"\n    type: directory-added\n    details:\n      path: \"vacant/nested\"\n"
    );
    assert!(!document.contains("target"));

    let applied = run(&["--apply", output_arg, edited_arg]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(!edited.join("gone").exists());
    assert!(edited.join("target/skipped").is_dir());
    assert!(edited.join("vacant/nested").is_dir());
    assert_eq!(fs::read(edited.join("keep.txt")).unwrap(), b"same\n");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn removes_a_directory_left_empty_by_a_rename() {
    let root = std::env::temp_dir().join(format!("semdiff-dir-rename-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(left.join("old")).unwrap();
    fs::create_dir_all(right.join("new")).unwrap();
    fs::create_dir_all(edited.join("old")).unwrap();
    let note = "moved\n";
    fs::write(left.join("old/note.txt"), note).unwrap();
    fs::write(right.join("new/note.txt"), note).unwrap();
    fs::write(edited.join("old/note.txt"), note).unwrap();

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
        "    type: file-renamed\n    details:\n      from: \"old/note.txt\"\n      to: \"new/note.txt\"\n"
    ));
    assert!(document.contains("    type: directory-deleted\n    details:\n      path: \"old\"\n"));
    assert!(!document.contains("directory-added"));

    let applied = run(&["--apply", output_arg, edited_arg]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert!(!Path::new(edited_arg).join("old").exists());
    assert_eq!(
        fs::read(Path::new(edited_arg).join("new/note.txt")).unwrap(),
        note.as_bytes()
    );

    let _ = fs::remove_dir_all(root);
}
