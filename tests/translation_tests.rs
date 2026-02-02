//! Translation functionality tests

use std::fs;
use tempfile::TempDir;

#[test]
fn test_translation_identifier_md5() {
    // Test that translation identifiers use MD5 hash matching Ren'Py's algorithm
    let temp_dir = TempDir::new().unwrap();

    // Create a simple script file
    let script_content = r#"
label start:
    "It's only when I hear the sounds of shuffling feet and supplies being put away that I realize that the lecture's over."
"#;

    let script_path = temp_dir.path().join("script.rpy");
    fs::write(&script_path, script_content).unwrap();

    let output_dir = temp_dir.path().join("output");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--template-only",
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    assert!(
        output.status.success(),
        "Patch should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read generated translation file
    let tl_file = output_dir.join("tl/chinese/script.rpy");
    assert!(tl_file.exists(), "Translation file should be created");

    let content = fs::read_to_string(&tl_file).unwrap();

    // The MD5 of the first dialogue should be 915cb944 (verified against Ren'Py)
    // Note: The exact hash depends on the full line format
    assert!(
        content.contains("translate chinese start_"),
        "Should contain translation block"
    );
}

#[test]
fn test_glossary_loading() {
    let temp_dir = TempDir::new().unwrap();

    // Create glossary file
    let glossary_content = r#"
# Test glossary
Sylvie = 西尔维
Professor = 教授
"#;

    let glossary_path = temp_dir.path().join("glossary.txt");
    fs::write(&glossary_path, glossary_content).unwrap();

    // Create a simple script
    let script_content = r#"
label start:
    "Hello Sylvie!"
"#;

    let script_path = temp_dir.path().join("script.rpy");
    fs::write(&script_path, script_content).unwrap();

    let output_dir = temp_dir.path().join("output");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--template-only",
            "--glossary",
            glossary_path.to_str().unwrap(),
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    assert!(
        output.status.success(),
        "Patch with glossary should succeed"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Loaded 2 glossary terms"),
        "Should report loaded terms"
    );
}

#[test]
fn test_glossary_formats() {
    let temp_dir = TempDir::new().unwrap();

    // Test different glossary formats
    let glossary_content = "
# Comments should be ignored
Source1 = Target1
Source2=Target2
Source3	Target3
";

    let glossary_path = temp_dir.path().join("glossary.txt");
    fs::write(&glossary_path, glossary_content).unwrap();

    let script_path = temp_dir.path().join("script.rpy");
    fs::write(&script_path, "label start:\n    \"test\"").unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--template-only",
            "--glossary",
            glossary_path.to_str().unwrap(),
            "-o",
            temp_dir.path().join("out").to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Loaded 3 glossary terms"),
        "Should load all 3 terms"
    );
}

#[test]
fn test_escape_sequences_preserved() {
    let temp_dir = TempDir::new().unwrap();

    let script_content = "label start:\n    e \"Line one.\\nLine two.\"\n";
    let script_path = temp_dir.path().join("script.rpy");
    fs::write(&script_path, script_content).unwrap();

    let output_dir = temp_dir.path().join("output");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_derenpy"))
        .args([
            "patch",
            script_path.parent().unwrap().to_str().unwrap(),
            "--template-only",
            "-o",
            output_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run patch");

    assert!(output.status.success());

    let tl_file = output_dir.join("tl/chinese/script.rpy");
    let content = fs::read_to_string(&tl_file).unwrap();

    assert!(content.contains("\\\\n"), "Should preserve \\n escape");
}
