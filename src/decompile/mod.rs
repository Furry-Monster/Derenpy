pub mod rpyc;

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use walkdir::WalkDir;

use crate::cli::DecompileArgs;
use rpyc::RpycDecompiler;

pub fn run(args: DecompileArgs) -> Result<()> {
    let input = &args.input;

    let decompiler = RpycDecompiler::new().context("Failed to initialize decompiler")?;

    if input.is_file() {
        decompile_single(&decompiler, input, args.output.as_deref(), args.force)?;
    } else if input.is_dir() {
        decompile_directory(
            &decompiler,
            input,
            args.output.as_deref(),
            args.recursive,
            args.force,
        )?;
    } else {
        anyhow::bail!("Input path does not exist: {}", input.display());
    }

    Ok(())
}

fn decompile_single(
    decompiler: &RpycDecompiler,
    input: &Path,
    output: Option<&Path>,
    force: bool,
) -> Result<()> {
    println!("{}", format!("[Decompile] {}", input.display()).green());

    let output_path = match output {
        Some(p) => {
            if p.is_dir() {
                let filename = input.file_stem().unwrap_or_default();
                p.join(filename).with_extension("rpy")
            } else {
                p.to_path_buf()
            }
        }
        None => {
            if input.extension().map(|e| e == "rpyc").unwrap_or(false) {
                input.with_extension("rpy")
            } else if input.extension().map(|e| e == "rpymc").unwrap_or(false) {
                input.with_extension("rpym")
            } else {
                input.with_extension("rpy")
            }
        }
    };

    if output_path.exists() && !force {
        anyhow::bail!(
            "Output file already exists: {} (use -f to overwrite)",
            output_path.display()
        );
    }

    let result = decompiler.decompile(input, Some(&output_path))?;
    println!("{}", format!("[OK] {}", result.display()).green());

    Ok(())
}

fn decompile_directory(
    decompiler: &RpycDecompiler,
    dir: &Path,
    output: Option<&Path>,
    recursive: bool,
    force: bool,
) -> Result<()> {
    let walker = if recursive {
        WalkDir::new(dir)
    } else {
        WalkDir::new(dir).max_depth(1)
    };

    let rpyc_files: Vec<_> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str());
            matches!(ext, Some("rpyc") | Some("rpymc"))
        })
        .collect();

    if rpyc_files.is_empty() {
        println!("{}", "[WARN] No RPYC files found".yellow());
        return Ok(());
    }

    println!(
        "{}",
        format!("[Decompile] Found {} RPYC file(s)", rpyc_files.len()).green()
    );

    let pb = ProgressBar::new(rpyc_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    let mut success_count = 0;
    let mut error_count = 0;

    for entry in rpyc_files {
        let rpyc_path = entry.path();
        pb.set_message(
            rpyc_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        );

        let out_path = match output {
            Some(base) => {
                let rel = rpyc_path.strip_prefix(dir).unwrap_or(rpyc_path);
                let new_ext = if rpyc_path.extension().map(|e| e == "rpymc").unwrap_or(false) {
                    "rpym"
                } else {
                    "rpy"
                };
                base.join(rel).with_extension(new_ext)
            }
            None => {
                let new_ext = if rpyc_path.extension().map(|e| e == "rpymc").unwrap_or(false) {
                    "rpym"
                } else {
                    "rpy"
                };
                rpyc_path.with_extension(new_ext)
            }
        };

        if out_path.exists() && !force {
            pb.inc(1);
            continue;
        }

        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match decompiler.decompile(rpyc_path, Some(&out_path)) {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_count += 1;
                pb.suspend(|| {
                    eprintln!(
                        "{}",
                        format!("[ERROR] {}: {}", rpyc_path.display(), e).red()
                    );
                });
            }
        }

        pb.inc(1);
    }

    pb.finish_and_clear();

    println!(
        "{}",
        format!(
            "[OK] Decompiled {} file(s), {} error(s)",
            success_count, error_count
        )
        .green()
    );

    Ok(())
}
