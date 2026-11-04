use super::run;

use std::fs;

#[test]
fn requires_an_output_path() {
    let output = run(&[]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr,
        "Usage: semdiff <left> <right> <output>\n       semdiff --apply <semdif> <directory>\n"
    );
}

#[test]
fn rejects_a_missing_source() {
    let output = run(&["missing-left", "missing-right", "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, "not a file or directory: missing-left\n");
}

#[test]
fn rejects_a_file_compared_with_a_directory() {
    let root = std::env::temp_dir().join(format!("semdiff-mixed-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();

    let file = file.to_str().unwrap();
    let directory = root.to_str().unwrap();
    let output = run(&[file, directory, "out.yml"]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr,
        "sources must both be files or both be directories\n"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_apply_to_a_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-file-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let file = root.join("quote.txt");
    let semdif = root.join("out.yml");
    fs::write(&file, "Brevity is the soul of wit.\n").unwrap();
    fs::write(&semdif, "header:\n  semdiff: \"0.1.0\"\n").unwrap();

    let file = file.to_str().unwrap();
    let semdif = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif, file]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a directory: {file}\n"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_a_missing_semdif_file() {
    let root = std::env::temp_dir().join(format!("semdiff-apply-missing-{}", std::process::id()));
    let source = root.join("source");
    fs::create_dir_all(&source).unwrap();
    let semdif = root.join("missing.yml");

    let source = source.to_str().unwrap();
    let semdif_arg = semdif.to_str().unwrap();
    let output = run(&["--apply", semdif_arg, source]);
    let stderr = String::from_utf8(output.stderr).expect("stderr was not utf-8");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr, format!("not a file: {}\n", semdif.display()));

    let _ = fs::remove_dir_all(root);
}
