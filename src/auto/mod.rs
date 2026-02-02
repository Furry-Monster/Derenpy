//! Auto workflow: unpack, decompile, and translate in one command

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::cli::{AutoArgs, DecompileArgs, PatchArgs};
use crate::decompile;
use crate::patch;
use crate::unpack::rpa::RpaArchive;

pub fn run(args: AutoArgs) -> Result<()> {
    println!(
        "{}",
        "[Auto] Starting automatic translation workflow".green()
    );

    let input = &args.input;
    let temp_dir = std::env::temp_dir().join(format!("derenpy_auto_{}", std::process::id()));
    let mut work_dir = input.clone();
    let mut cleanup_dirs: Vec<PathBuf> = Vec::new();

    // Step 1: Unpack RPA if needed
    if is_rpa_file(input) {
        println!("\n{}", "[Step 1/3] Unpacking RPA archive...".cyan());
        let extract_dir = temp_dir.join("extracted");
        fs::create_dir_all(&extract_dir)?;

        let archive = RpaArchive::open(input).context("Failed to open RPA archive")?;

        println!(
            "  Version: {}, Files: {}",
            archive.version,
            archive.file_count()
        );
        archive.extract_all(&extract_dir, None)?;
        println!("  Extracted to: {}", extract_dir.display());

        work_dir = extract_dir.clone();
        if !args.keep_temp {
            cleanup_dirs.push(temp_dir.clone());
        }
    } else if input.is_dir() {
        println!("\n{}", "[Step 1/3] Using directory as input".cyan());
        println!("  Path: {}", input.display());
    } else {
        anyhow::bail!("Input must be an RPA file or directory");
    }

    // Step 2: Decompile RPYC files if needed
    let rpyc_files = find_rpyc_files(&work_dir);
    let rpy_files = find_rpy_files(&work_dir);

    if !rpyc_files.is_empty() && rpy_files.is_empty() {
        println!("\n{}", "[Step 2/3] Decompiling RPYC scripts...".cyan());
        println!("  Found {} RPYC file(s)", rpyc_files.len());

        let decompile_args = DecompileArgs {
            input: work_dir.clone(),
            output: None,
            recursive: true,
            force: true,
        };

        decompile::run(decompile_args)?;
    } else if !rpy_files.is_empty() {
        println!(
            "\n{}",
            "[Step 2/3] RPY files found, skipping decompilation".cyan()
        );
        println!("  Found {} RPY file(s)", rpy_files.len());
    } else {
        println!("\n{}", "[Step 2/3] No scripts found".yellow());
    }

    // Step 3: Generate translation patch
    println!("\n{}", "[Step 3/3] Generating translation patch...".cyan());

    let output_dir = args.output.unwrap_or_else(|| {
        if args.input.is_dir() {
            args.input.join("game")
        } else {
            let stem = args
                .input
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "game".to_string());
            PathBuf::from(format!("{}_translation", stem))
        }
    });

    let patch_args = PatchArgs {
        input: work_dir,
        output: Some(output_dir.clone()),
        lang: args.lang,
        api: args.api,
        api_key: args.api_key,
        api_base: args.api_base,
        model: args.model,
        template_only: args.template_only,
        glossary: args.glossary,
    };

    patch::run(patch_args)?;

    // Cleanup temporary files
    if !args.keep_temp {
        for dir in cleanup_dirs {
            if dir.exists() {
                let _ = fs::remove_dir_all(&dir);
            }
        }
    }

    println!("\n{}", "[Auto] Workflow completed!".green().bold());
    println!("  Output: {}", output_dir.display());

    Ok(())
}

fn is_rpa_file(path: &Path) -> bool {
    path.is_file() && path.extension().map(|e| e == "rpa").unwrap_or(false)
}

fn find_rpyc_files(dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "rpyc" || ext == "rpymc")
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn find_rpy_files(dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "rpy" || ext == "rpym")
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}
