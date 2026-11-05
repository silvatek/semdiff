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
        "  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"drop.txt\"\n      old: \"3282251a9b2bf054b863c3712910d292ff2f0c57d0fb16bb43c50157b20cce56\"\n      new: \"f660a7996deacfbc7560e4240054a8ad82eb02fe25a95064257e07084bcacb85\"\n  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"drop.txt\"\n      old: 2\n      lines: -1\n      new: 2\n      text: \"gone\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"note.txt\"\n      old: \"9e9894bfcaf8d63b9898e90db0fe05f5a0388d45bcf60a0a208ad31153072142\"\n      new: \"f77de5e87174686ce6fca0e3d03893b38dc6020febaa55578a8b20b289b5c362\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"data.bin\"\n      old: \"cba06b5736faf67e54b07b561eae94395e774c517a7d910a54369e1263ccfbd4\"\n      new: \"06eb7d6a69ee19e5fbdf749018d3d2abfa04bcbd1365db312eb86dc7169389b8\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-modified\n    details:\n      path: \"note.txt\"\n      old: 1\n      lines: +0\n      new: 1\n      text: \"He said \\\"hello\\\"\"\n  - scope: \".\"\n    type: line-added\n    details:\n      path: \"note.txt\"\n      old: 2\n      lines: +1\n      new: 2\n      text: \"Next line\"\n"
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
        "  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"edit.txt\"\n      old: \"38a0e71cdc417583a6f82094581552227c96aaa4dbb897edece3f69a71d31464\"\n      new: \"21f14a6d40bb2fcf5c54176224f1ae9cfb50dad1e6561d62c2cadff3bd848ca2\"\n  - scope: \".\"\n    type: line-modified\n    details:\n      path: \"edit.txt\"\n      old: 2\n      lines: +0\n      new: 2\n      text: \"new\"\n  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"edit.txt\"\n      old: 3\n      lines: -1\n      new: 3\n      text: \"gone\"\n  - scope: \".\"\n    type: line-added\n    details:\n      path: \"edit.txt\"\n      old: 5\n      lines: +1\n      new: 4\n      text: \"extra\"\n"
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
fn merges_adjacent_removed_lines_into_one_span() {
    let root = std::env::temp_dir().join(format!("semdiff-removed-span-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::create_dir_all(&edited).unwrap();
    fs::write(left.join("span.txt"), "keep\none\ntwo\nthree\ntail\n").unwrap();
    fs::write(right.join("span.txt"), "keep\ntail\n").unwrap();
    fs::write(edited.join("span.txt"), "keep\none\ntwo\nthree\ntail\n").unwrap();
    fs::write(left.join("gap.txt"), "keep\none\nmid\ntwo\ntail\n").unwrap();
    fs::write(right.join("gap.txt"), "keep\nmid\ntail\n").unwrap();
    fs::write(edited.join("gap.txt"), "keep\none\nmid\ntwo\ntail\n").unwrap();

    let output_path = root.join("out.yml");
    let output_arg = output_path.to_str().unwrap();
    let compared = run(&[left.to_str().unwrap(), right.to_str().unwrap(), output_arg]);
    let document = fs::read_to_string(&output_path).expect("failed to read semdiff file");

    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"span.txt\"\n      old: 2\n      lines: -3\n      new: 2\n      text: \"one\\ntwo\\nthree\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"gap.txt\"\n      old: 2\n      lines: -1\n      new: 2\n      text: \"one\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"gap.txt\"\n      old: 4\n      lines: -1\n      new: 3\n      text: \"two\"\n"
    ));

    let applied = run(&["--apply", output_arg, edited.to_str().unwrap()]);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(fs::read(edited.join("span.txt")).unwrap(), b"keep\ntail\n");
    assert_eq!(
        fs::read(edited.join("gap.txt")).unwrap(),
        b"keep\nmid\ntail\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn merges_the_adjacent_removals_in_the_rust_sample() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sample = manifest.join("TestCode/rust/main");
    let root = std::env::temp_dir().join(format!("semdiff-sample-span-{}", std::process::id()));
    let output_path = root.join("out.yml");
    fs::create_dir_all(&root).unwrap();
    let compared = run(&[
        sample.join("v1").to_str().unwrap(),
        sample.join("v2").to_str().unwrap(),
        output_path.to_str().unwrap(),
    ]);
    let document = fs::read_to_string(&output_path).expect("failed to read semdiff file");

    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"src/main.rs\"\n      old: 43\n      lines: -1\n      new: 44\n      text: \"// This single line is deleted.\"\n"
    ));
    assert!(document.contains(
        "  - scope: \".\"\n    type: line-removed\n    details:\n      path: \"src/main.rs\"\n      old: 49\n      lines: -2\n      new: 49\n      text: \"// This deleted line is first.\\n// This deleted line is second.\"\n"
    ));
    assert!(!document.contains("old: 50\n"), "{document}");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_line_removal_whose_line_count_is_positive() {
    let root = std::env::temp_dir().join(format!("semdiff-bad-span-{}", std::process::id()));
    let edited = root.join("edited");
    fs::create_dir_all(&edited).unwrap();
    fs::write(edited.join("note.txt"), "keep\ngone\n").unwrap();
    let semdif = root.join("bad.yml");
    fs::write(
        &semdif,
        "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: line-removed
    details:
      path: \"note.txt\"
      old: 2
      lines: 2
      new: 2
      text: \"gone\"
",
    )
    .unwrap();

    let applied = run(&[
        "--apply",
        semdif.to_str().unwrap(),
        edited.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8(applied.stderr).unwrap();

    assert_eq!(applied.status.code(), Some(1));
    assert!(stderr.contains("invalid lines 2"), "{stderr}");
    assert_eq!(fs::read(edited.join("note.txt")).unwrap(), b"keep\ngone\n");

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
        "  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"note.txt\"\n      old: \"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824\"\n      new: \"5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\"\n  - scope: \".\"\n    type: file-changed\n    details:\n      path: \"note.txt\"\n      contents: \"hello\\n\"\n"
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

#[test]
fn rejects_a_file_whose_old_hash_does_not_match() {
    let (root, edited, document) = changed_note("old");
    let wrong = "0".repeat(64);
    let old_hash = "9e9894bfcaf8d63b9898e90db0fe05f5a0388d45bcf60a0a208ad31153072142";
    let document = document.replacen(old_hash, &wrong, 1);
    let message = apply_document(&root, &edited, &document);
    assert!(message.contains("do not match the old hash"), "{message}");
    assert_eq!(fs::read(edited.join("note.txt")).unwrap(), b"old line\n");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_file_whose_new_hash_does_not_match() {
    let (root, edited, document) = changed_note("new");
    let wrong = "f".repeat(64);
    let new_hash = "f77de5e87174686ce6fca0e3d03893b38dc6020febaa55578a8b20b289b5c362";
    let document = document.replacen(new_hash, &wrong, 1);
    let message = apply_document(&root, &edited, &document);
    assert!(message.contains("do not match the new hash"), "{message}");
    assert_eq!(fs::read(edited.join("note.txt")).unwrap(), b"old line\n");
    let _ = fs::remove_dir_all(root);
}

fn changed_note(name: &str) -> (std::path::PathBuf, std::path::PathBuf, String) {
    let root = std::env::temp_dir().join(format!("semdiff-hash-{name}-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    let edited = root.join("edited");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();
    fs::create_dir_all(&edited).unwrap();
    fs::write(left.join("note.txt"), "old line\n").unwrap();
    fs::write(right.join("note.txt"), "He said \"hello\"\nNext line\n").unwrap();
    fs::write(edited.join("note.txt"), "old line\n").unwrap();
    let output_path = root.join("out.yml");
    let compared = run(&[
        left.to_str().unwrap(),
        right.to_str().unwrap(),
        output_path.to_str().unwrap(),
    ]);
    assert!(
        compared.status.success(),
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    let document = fs::read_to_string(&output_path).unwrap();
    (root, edited, document)
}

fn apply_document(root: &std::path::Path, edited: &std::path::Path, document: &str) -> String {
    let semdif = root.join("tampered.yml");
    fs::write(&semdif, document).unwrap();
    let applied = run(&[
        "--apply",
        semdif.to_str().unwrap(),
        edited.to_str().unwrap(),
    ]);
    assert_eq!(applied.status.code(), Some(1));
    String::from_utf8(applied.stderr).unwrap()
}
