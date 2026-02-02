mod auto;
mod cli;
mod config;
mod decompile;
mod patch;
mod repack;
mod translate;
mod unpack;
mod utils;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use cli::{Cli, Commands};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Unpack(args) => unpack::run(args)?,
        Commands::Decompile(args) => decompile::run(args)?,
        Commands::Translate(args) => translate::run(args)?,
        Commands::Repack(args) => repack::run(args)?,
        Commands::Patch(args) => patch::run(args)?,
        Commands::Config(args) => config::commands::run(args)?,
        Commands::Auto(args) => auto::run(args)?,
    }

    Ok(())
}
