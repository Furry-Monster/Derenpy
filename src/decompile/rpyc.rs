//! RPYC decompiler - Python bridge for unrpyc

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct DecompileResult {
    output: String,
    success: bool,
    error: Option<String>,
}

pub struct RpycDecompiler {
    python_path: String,
    script_path: PathBuf,
}

impl RpycDecompiler {
    pub fn new() -> Result<Self> {
        let script_path = Self::find_script_path()?;

        Ok(Self {
            python_path: "python3".to_string(),
            script_path,
        })
    }

    fn find_script_path() -> Result<PathBuf> {
        // Try to find the decompile.py script relative to the executable
        let exe_path = std::env::current_exe().context("Failed to get executable path")?;
        let exe_dir = exe_path.parent().unwrap_or(Path::new("."));

        // Check various possible locations
        let candidates = [
            exe_dir.join("scripts/decompile.py"),
            exe_dir.join("../scripts/decompile.py"),
            exe_dir.join("../../scripts/decompile.py"),
            PathBuf::from("scripts/decompile.py"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate.canonicalize()?);
            }
        }

        anyhow::bail!(
            "Could not find decompile.py script. Searched in: {:?}",
            candidates
        )
    }

    pub fn decompile<P: AsRef<Path>>(&self, input: P, output: Option<&Path>) -> Result<PathBuf> {
        let input = input.as_ref();

        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.script_path).arg(input);

        if let Some(out) = output {
            cmd.arg(out);
        }

        let output_result = cmd
            .output()
            .context("Failed to execute Python decompiler")?;

        let stdout = String::from_utf8_lossy(&output_result.stdout);

        if stdout.trim().is_empty() {
            if !output_result.status.success() {
                let stderr = String::from_utf8_lossy(&output_result.stderr);
                anyhow::bail!("Decompiler failed: {}", stderr);
            }
            anyhow::bail!("Decompiler produced no output");
        }

        let result: DecompileResult =
            serde_json::from_str(&stdout).context("Failed to parse decompiler output")?;

        if result.success {
            Ok(PathBuf::from(result.output))
        } else {
            anyhow::bail!(
                "Decompilation failed: {}",
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            )
        }
    }
}

impl Default for RpycDecompiler {
    fn default() -> Self {
        Self::new().expect("Failed to create RpycDecompiler")
    }
}
