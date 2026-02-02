pub mod rpa;

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use walkdir::WalkDir;

use crate::cli::UnpackArgs;
use rpa::RpaArchive;

pub fn run(args: UnpackArgs) -> Result<()> {
    let input = &args.input;

    if input.is_file() {
        unpack_single(input, args.output.as_deref(), args.force)?;
    } else if input.is_dir() {
        unpack_directory(input, args.output.as_deref(), args.recursive, args.force)?;
    } else {
        anyhow::bail!("Input path does not exist: {}", input.display());
    }

    Ok(())
}

fn unpack_single(input: &Path, output: Option<&Path>, force: bool) -> Result<()> {
    println!("{}", format!("[Unpack] {}", input.display()).green());

    let archive = RpaArchive::open(input).context("Failed to open RPA archive")?;

    println!(
        "  Version: {}, Files: {}",
        archive.version,
        archive.file_count()
    );

    let output_dir = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let stem = input.file_stem().unwrap_or_default();
            input.parent().unwrap_or(Path::new(".")).join(stem)
        }
    };

    if output_dir.exists() && !force {
        anyhow::bail!(
            "Output directory already exists: {} (use -f to overwrite)",
            output_dir.display()
        );
    }

    std::fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

    let pb = ProgressBar::new(archive.file_count() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    archive.extract_all(&output_dir, Some(&pb))?;

    pb.finish_with_message("done");
    println!(
        "{}",
        format!("[OK] Extracted to {}", output_dir.display()).green()
    );

    Ok(())
}

fn unpack_directory(dir: &Path, output: Option<&Path>, recursive: bool, force: bool) -> Result<()> {
    let walker = if recursive {
        WalkDir::new(dir)
    } else {
        WalkDir::new(dir).max_depth(1)
    };

    let rpa_files: Vec<_> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("rpa"))
                .unwrap_or(false)
        })
        .collect();

    if rpa_files.is_empty() {
        println!("{}", "[WARN] No RPA files found".yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("[Unpack] Found {} RPA file(s)", rpa_files.len()).green()
    );

    for entry in rpa_files {
        let rpa_path = entry.path();
        let out_dir = match output {
            Some(base) => {
                let rel = rpa_path.strip_prefix(dir).unwrap_or(rpa_path);
                let stem = rel.file_stem().unwrap_or_default();
                base.join(stem)
            }
            None => {
                let stem = rpa_path.file_stem().unwrap_or_default();
                rpa_path.parent().unwrap_or(Path::new(".")).join(stem)
            }
        };

        if let Err(e) = unpack_single(rpa_path, Some(&out_dir), force) {
            eprintln!(
                "{}",
                format!("[ERROR] Failed to unpack {}: {}", rpa_path.display(), e).red()
            );
        }
    }

    Ok(())
}
