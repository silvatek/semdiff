use super::run;

use std::fs;

#[test]
fn prints_name_and_version_when_comparing_directories() {
    let root = std::env::temp_dir().join(format!("semdiff-cli-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    fs::create_dir_all(&left).unwrap();
    fs::create_dir_all(&right).unwrap();

    let output_path = root.join("out.yml");
    let left = left.to_str().unwrap();
    let right = right.to_str().unwrap();
    let output_path = output_path.to_str().unwrap();
    let output = run(&[left, right, output_path]);
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    let document = fs::read_to_string(output_path).expect("failed to read semdiff file");

    assert!(output.status.success());
    assert_eq!(
        stdout,
        format!(
            "Semdiff {}\nComparing {left} and {right}\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    let version_line = format!("  semdiff: \"{}\"", env!("CARGO_PKG_VERSION"));
    let mut lines = document.lines();
    assert_eq!(lines.next(), Some("header:"));
    assert_eq!(lines.next(), Some(version_line.as_str()));
    let timestamp = lines
        .next()
        .and_then(|line| line.strip_prefix("  timestamp: \""))
        .and_then(|value| value.strip_suffix('"'))
        .expect("timestamp line");
    assert_eq!(timestamp.len(), 20, "{timestamp}");
    assert_eq!(&timestamp[4..5], "-");
    assert_eq!(&timestamp[10..11], "T");
    assert!(timestamp.ends_with('Z'), "{timestamp}");
    assert!(lines.next().is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_change_that_is_not_a_rename() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-type-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    let semdif = root.join("out.yml");
    fs::write(
        &semdif,
        "\
header:
  semdiff: \"0.1.0\"
  timestamp: \"2020-01-01T00:00:00Z\"
changes:
  - scope: \".\"
    type: line-comment-added
    details:
      line: 1
      text: \"/// note\"
",
    )
    .unwrap();

    let source = source.to_str().unwrap();
    let semdif = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif, source]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, "unknown change type line-comment-added\n");

    let _ = fs::remove_dir_all(root);
}
