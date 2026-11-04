use std::process::Command;

#[test]
fn prints_name_and_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_semdiff"))
        .output()
        .expect("failed to run semdiff");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout was not utf-8");
    assert_eq!(stdout, format!("Semdiff {}\n", env!("CARGO_PKG_VERSION")));
}
