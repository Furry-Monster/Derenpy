//! RPA archive repacking

pub mod rpa;

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

use crate::cli::RepackArgs;
use rpa::RpaWriter;

pub fn run(args: RepackArgs) -> Result<()> {
    let input = &args.input;

    if !input.is_dir() {
        anyhow::bail!("Input must be a directory: {}", input.display());
    }

    let output = args.output.unwrap_or_else(|| input.with_extension("rpa"));

    println!("{}", format!("[Repack] {}", input.display()).green());

    // Collect all files
    let files: Vec<_> = WalkDir::new(input)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();

    if files.is_empty() {
        anyhow::bail!("No files found in directory");
    }

    println!("  Found {} file(s)", files.len());

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")?
            .progress_chars("=>-"),
    );

    let mut writer = RpaWriter::new(&output, args.version.as_deref().unwrap_or("3.0"))?;

    for entry in &files {
        let file_path = entry.path();
        let relative = file_path.strip_prefix(input).unwrap_or(file_path);

        pb.set_message(relative.to_string_lossy().to_string());

        writer
            .add_file(file_path, relative)
            .context(format!("Failed to add file: {}", file_path.display()))?;

        pb.inc(1);
    }

    writer.finish()?;

    pb.finish_and_clear();

    println!("{}", format!("[OK] Created {}", output.display()).green());

    Ok(())
}
