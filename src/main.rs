mod categorizer;
mod client;
mod config;
mod enhancer;
mod processor;
mod types;
mod writer;

use config::Config;
use processor::Processor;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    let config = Config::parse_args();

    // Setup logging
    let log_level = if config.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();

    info!("notex - AI-powered note compressor");
    info!("Input: {:?}", config.input);
    info!("Output: {:?}", config.output);
    info!("Model: {} @ {}", config.model, config.url);
    info!("Parallel: {} | Retries: {}", config.parallel, config.retries);
    info!("Format: {:?}", config.format);

    if config.dry_run {
        info!("Mode: DRY RUN (no files will be written)");
    }
    if !config.exclude.is_empty() {
        info!("Excluding: {:?}", config.exclude);
    }
    if config.reorganize {
        info!("Reorganization pass: ENABLED");
    }
    if config.cross_ref {
        info!("Cross-referencing: ENABLED");
    }

    let processor = Processor::new(config.clone());

    match processor.run().await {
        Ok(files) => {
            if !config.dry_run {
                info!("Successfully processed notes!");
                for file in &files {
                    println!("  {}", file.display());
                }
                println!("\nWrote {} files", files.len());
            }
        }
        Err(e) => {
            error!("Processing failed: {}", e);
            std::process::exit(1);
        }
    }
}
