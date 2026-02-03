use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "derenpy")]
#[command(author, version, about = "Renpy game reverse engineering and translation toolkit", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Unpack RPA archive files
    Unpack(UnpackArgs),

    /// Decompile RPYC script files
    Decompile(DecompileArgs),

    /// AI-powered game script translation
    Translate(TranslateArgs),

    /// Repack files into RPA archive
    Repack(RepackArgs),

    /// Generate translation patch for a game
    Patch(PatchArgs),

    /// Manage configuration
    Config(ConfigArgs),

    /// Auto workflow: unpack, decompile, and translate in one command
    Auto(AutoArgs),
}

#[derive(Parser, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,

    /// Initialize configuration file with defaults
    Init {
        /// Overwrite existing config
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., api.openai_api_key)
        key: String,
        /// Value to set
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Show config file path
    Path,

    /// Edit config file with default editor
    Edit,
}

#[derive(Parser, Debug)]
pub struct UnpackArgs {
    /// Input RPA file or directory containing RPA files
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Process subdirectories recursively
    #[arg(short, long, default_value_t = false)]
    pub recursive: bool,

    /// Overwrite existing files
    #[arg(short, long, default_value_t = false)]
    pub force: bool,
}

#[derive(Parser, Debug)]
pub struct DecompileArgs {
    /// Input RPYC file or directory containing RPYC files
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Process subdirectories recursively
    #[arg(short, long, default_value_t = false)]
    pub recursive: bool,

    /// Overwrite existing files
    #[arg(short, long, default_value_t = false)]
    pub force: bool,
}

#[derive(Parser, Debug)]
pub struct TranslateArgs {
    /// Input script file or directory
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Target language (e.g., zh-CN, en, ja)
    #[arg(short, long, default_value = "zh-CN")]
    pub lang: String,

    /// API provider (openai, claude, ollama)
    #[arg(long, default_value = "openai")]
    pub api: String,

    /// API key (can also be set via environment variable)
    #[arg(long)]
    pub api_key: Option<String>,

    /// API base URL (for custom endpoints)
    #[arg(long)]
    pub api_base: Option<String>,

    /// Model name to use
    #[arg(long)]
    pub model: Option<String>,

    /// Process subdirectories recursively
    #[arg(short, long, default_value_t = false)]
    pub recursive: bool,

    /// Generate Renpy translation files instead of modifying source
    #[arg(long, default_value_t = false)]
    pub patch_mode: bool,
}

#[derive(Parser, Debug)]
pub struct RepackArgs {
    /// Input directory to pack
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output RPA file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// RPA version (2.0 or 3.0)
    #[arg(long)]
    pub version: Option<String>,
}

#[derive(Parser, Debug)]
pub struct PatchArgs {
    /// Game directory or RPA file
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output directory for the patch
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Target language code (e.g., chinese, japanese, korean)
    #[arg(short, long, default_value = "chinese")]
    pub lang: String,

    /// API provider (openai, claude, ollama)
    #[arg(long, default_value = "openai")]
    pub api: String,

    /// API key
    #[arg(long)]
    pub api_key: Option<String>,

    /// API base URL
    #[arg(long)]
    pub api_base: Option<String>,

    /// Model name
    #[arg(long)]
    pub model: Option<String>,

    /// Skip translation, only generate template files
    #[arg(long, default_value_t = false)]
    pub template_only: bool,

    /// Glossary file for consistent term translation
    #[arg(long)]
    pub glossary: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct AutoArgs {
    /// Game directory or RPA file
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output directory for the translation patch
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Target language code (e.g., chinese, japanese, korean)
    #[arg(short, long, default_value = "chinese")]
    pub lang: String,

    /// API provider (openai, claude, ollama, google, deepl)
    #[arg(long, default_value = "google")]
    pub api: String,

    /// API key
    #[arg(long)]
    pub api_key: Option<String>,

    /// API base URL
    #[arg(long)]
    pub api_base: Option<String>,

    /// Model name
    #[arg(long)]
    pub model: Option<String>,

    /// Skip translation, only generate template files
    #[arg(long, default_value_t = false)]
    pub template_only: bool,

    /// Keep temporary files (extracted RPA, decompiled scripts)
    #[arg(long, default_value_t = false)]
    pub keep_temp: bool,

    /// Glossary file for consistent term translation
    #[arg(long)]
    pub glossary: Option<PathBuf>,
}
