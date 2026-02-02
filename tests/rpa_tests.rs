//! RPA archive tests

use std::fs;
use tempfile::TempDir;

#[test]
fn test_rpa_roundtrip() {
    let temp_dir = TempDir::new().unwrap();

    // Create source directory with test file
    let source_dir = temp_dir.path().join("source");
    fs::create_dir(&source_dir).unwrap();

    let test_file = source_dir.join("test.txt");
    let test_content = "Hello, Ren'Py!";
    fs::write(&test_file, test_content).unwrap();

    // Create RPA archive
    let rpa_path = temp_dir.path().join("test.rpa");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "repack",
            source_dir.to_str().unwrap(),
            "-o",
            rpa_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run repack");

    assert!(status.success(), "Repack should succeed");
    assert!(rpa_path.exists(), "RPA file should be created");

    // Extract RPA archive (use -f to overwrite)
    let extract_dir = temp_dir.path().join("extracted");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "unpack",
            rpa_path.to_str().unwrap(),
            "-o",
            extract_dir.to_str().unwrap(),
            "-f",
        ])
        .status()
        .expect("Failed to run unpack");

    assert!(status.success(), "Unpack should succeed");

    // Verify extracted content
    let extracted_file = extract_dir.join("test.txt");
    assert!(extracted_file.exists(), "Extracted file should exist");

    let extracted_content = fs::read_to_string(&extracted_file).unwrap();
    assert_eq!(extracted_content, test_content, "Content should match");
}

#[test]
fn test_rpa_version_header() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("dummy.txt");
    fs::write(&test_file, "test").unwrap();

    let rpa_path = temp_dir.path().join("test.rpa");

    std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "repack",
            test_file.parent().unwrap().to_str().unwrap(),
            "-o",
            rpa_path.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run repack");

    // Read header
    let content = fs::read(&rpa_path).unwrap();
    let header = String::from_utf8_lossy(&content[..7]);

    assert!(
        header.starts_with("RPA-3.0"),
        "Should create RPA-3.0 by default"
    );
}
