//! CLI interface tests

use std::process::Command;

fn derenpy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_derenpy"))
}

#[test]
fn test_help_command() {
    let output = derenpy()
        .arg("--help")
        .output()
        .expect("Failed to run help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("unpack"), "Should list unpack command");
    assert!(stdout.contains("repack"), "Should list repack command");
    assert!(
        stdout.contains("decompile"),
        "Should list decompile command"
    );
    assert!(
        stdout.contains("translate"),
        "Should list translate command"
    );
    assert!(stdout.contains("patch"), "Should list patch command");
    assert!(stdout.contains("auto"), "Should list auto command");
    assert!(stdout.contains("config"), "Should list config command");
}

#[test]
fn test_version_command() {
    let output = derenpy()
        .arg("--version")
        .output()
        .expect("Failed to run version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("derenpy"), "Should show program name");
}

#[test]
fn test_unpack_help() {
    let output = derenpy()
        .args(["unpack", "--help"])
        .output()
        .expect("Failed to run unpack help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--output"), "Should have output option");
    assert!(
        stdout.contains("--recursive"),
        "Should have recursive option"
    );
}

#[test]
fn test_patch_help() {
    let output = derenpy()
        .args(["patch", "--help"])
        .output()
        .expect("Failed to run patch help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--api"), "Should have api option");
    assert!(stdout.contains("--lang"), "Should have lang option");
    assert!(stdout.contains("--glossary"), "Should have glossary option");
    assert!(
        stdout.contains("--template-only"),
        "Should have template-only option"
    );
}

#[test]
fn test_auto_help() {
    let output = derenpy()
        .args(["auto", "--help"])
        .output()
        .expect("Failed to run auto help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--api"), "Should have api option");
    assert!(stdout.contains("--glossary"), "Should have glossary option");
    assert!(
        stdout.contains("--keep-temp"),
        "Should have keep-temp option"
    );
}

#[test]
fn test_invalid_command() {
    let output = derenpy()
        .arg("invalid_command")
        .output()
        .expect("Failed to run invalid command");

    assert!(!output.status.success(), "Should fail on invalid command");
}

#[test]
fn test_missing_input() {
    let output = derenpy()
        .arg("unpack")
        .output()
        .expect("Failed to run unpack without input");

    assert!(!output.status.success(), "Should fail without input");
}
