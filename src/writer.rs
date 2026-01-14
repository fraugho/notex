use crate::types::{EnhancedSegment, OutputFormat};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WriterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Group enhanced segments by their output paths
pub fn group_by_output_path(
    segments: Vec<EnhancedSegment>,
) -> HashMap<String, Vec<EnhancedSegment>> {
    let mut grouped: HashMap<String, Vec<EnhancedSegment>> = HashMap::new();

    for segment in segments {
        for path in &segment.output_paths {
            grouped
                .entry(path.clone())
                .or_default()
                .push(segment.clone());
        }
    }

    grouped
}

/// Write all grouped segments to output directory
pub fn write_outputs(
    output_dir: &Path,
    grouped: HashMap<String, Vec<EnhancedSegment>>,
    format: OutputFormat,
) -> Result<Vec<PathBuf>, WriterError> {
    let mut written_files = Vec::new();

    for (rel_path, segments) in grouped {
        let file_path = output_dir.join(&rel_path);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Build file content
        let content = build_file_content(&segments, format);

        // Write file
        fs::write(&file_path, content)?;
        written_files.push(file_path);
    }

    Ok(written_files)
}

fn build_file_content(segments: &[EnhancedSegment], format: OutputFormat) -> String {
    let mut content = String::new();

    match format {
        OutputFormat::Markdown => {
            for (i, segment) in segments.iter().enumerate() {
                if i > 0 {
                    content.push_str("\n\n---\n\n");
                }
                content.push_str(&segment.content);
            }
        }
        OutputFormat::Plain => {
            for (i, segment) in segments.iter().enumerate() {
                if i > 0 {
                    content.push_str("\n\n");
                    content.push_str(&"=".repeat(80));
                    content.push_str("\n\n");
                }
                content.push_str(&segment.content);
            }
        }
    }

    // Ensure file ends with newline
    if !content.ends_with('\n') {
        content.push('\n');
    }

    content
}
