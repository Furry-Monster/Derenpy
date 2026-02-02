//! Translation cache tests

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn get_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap()
        .join("derenpy")
        .join("translations.db")
}

#[test]
fn test_cache_creates_database() {
    // Clear existing cache
    let cache_path = get_cache_path();
    if cache_path.exists() {
        fs::remove_file(&cache_path).ok();
    }

    let temp_dir = TempDir::new().unwrap();

    // Create a simple script
    let script_content = r#"
label start:
    "Test dialogue"
"#;

    let script_path = temp_dir.path().join("script.rpy");
    fs::write(&script_path, script_content).unwrap();

    // Run patch with Google Translate (free)
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--api",
            "google",
            "-o",
            temp_dir.path().join("out").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    assert!(output.status.success(), "Patch should succeed");

    // Cache database should be created
    assert!(cache_path.exists(), "Cache database should be created");
}

#[test]
fn test_cache_hit_on_repeated_translation() {
    let temp_dir = TempDir::new().unwrap();

    let script_content = r#"
label start:
    "Hello world"
"#;

    let script_path = temp_dir.path().join("script.rpy");
    fs::write(&script_path, script_content).unwrap();

    // First run - should make API call
    let output1 = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--api",
            "google",
            "-o",
            temp_dir.path().join("out1").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    assert!(output1.status.success());
    let _stdout1 = String::from_utf8_lossy(&output1.stdout);

    // Second run - should hit cache
    let output2 = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--api",
            "google",
            "-o",
            temp_dir.path().join("out2").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    // Second run should show cache hits
    assert!(
        stdout2.contains("cached"),
        "Second run should show cache hits"
    );
}
