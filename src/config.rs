use crate::types::OutputFormat;
use clap::Parser;
use std::path::PathBuf;

/// Notex - AI-powered note compressor and enhancer
#[derive(Parser, Debug, Clone)]
#[command(name = "notex")]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Input directory containing notes to process
    #[arg(value_name = "INPUT_DIR")]
    pub input: PathBuf,

    /// Output directory for processed notes
    #[arg(short, long, default_value = "./compressed")]
    pub output: PathBuf,

    /// Model name to use
    #[arg(short, long, default_value = "gpt-3.5-turbo")]
    pub model: String,

    /// API base URL (e.g., http://localhost:8080/v1 for llama-server)
    #[arg(short = 'u', long, default_value = "http://localhost:8080/v1")]
    pub url: String,

    /// API key (use "sk-no-key-required" for local servers)
    #[arg(short = 'k', long, default_value = "sk-no-key-required")]
    pub api_key: String,

    /// Maximum concurrent LLM requests (match your server's -np value)
    #[arg(short, long, default_value = "8")]
    pub parallel: usize,

    /// Output format
    #[arg(short, long, value_enum, default_value = "markdown")]
    pub format: OutputFormat,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Dry run - only categorize and show plan, don't enhance or write
    #[arg(long)]
    pub dry_run: bool,

    /// Exclude patterns (glob syntax, can be specified multiple times)
    #[arg(short = 'x', long = "exclude", value_name = "PATTERN")]
    pub exclude: Vec<String>,

    /// Number of retries for failed LLM calls
    #[arg(long, default_value = "3")]
    pub retries: usize,

    /// Run reorganization pass to optimize file structure
    #[arg(long)]
    pub reorganize: bool,

    /// Add cross-references between related notes
    #[arg(long)]
    pub cross_ref: bool,
}

impl Config {
    pub fn parse_args() -> Self {
        Config::parse()
    }
}
