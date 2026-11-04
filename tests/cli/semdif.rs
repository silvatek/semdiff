use std::fs;

use super::run;

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
    let document = fs::read_to_string(output_path).expect("failed to read semdif file");

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
