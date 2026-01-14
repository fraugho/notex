use crate::categorizer::{categorize_note, CategorizationError};
use crate::client::LlmClient;
use crate::config::Config;
use crate::enhancer::{enhance_segment, EnhancementError};
use crate::types::{CrossRefResponse, EnhancedSegment, RawNote, ReorgResponse, Segment};
use crate::writer::{group_by_output_path, write_outputs, WriterError};
use futures::stream::{self, StreamExt};
use glob::Pattern;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum ProcessorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Categorization error: {0}")]
    Categorization(#[from] CategorizationError),
    #[error("Enhancement error: {0}")]
    Enhancement(#[from] EnhancementError),
    #[error("Writer error: {0}")]
    Writer(#[from] WriterError),
}

/// Main processor that orchestrates the entire pipeline
pub struct Processor {
    client: LlmClient,
    config: Config,
    semaphore: Arc<Semaphore>,
    exclude_patterns: Vec<Pattern>,
}

impl Processor {
    pub fn new(config: Config) -> Self {
        let client = LlmClient::new(&config.url, &config.api_key, &config.model, config.retries);
        let semaphore = Arc::new(Semaphore::new(config.parallel));

        // Parse exclude patterns
        let exclude_patterns: Vec<Pattern> = config
            .exclude
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        Self {
            client,
            config,
            semaphore,
            exclude_patterns,
        }
    }

    /// Check if a path should be excluded
    fn is_excluded(&self, path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();
        self.exclude_patterns
            .iter()
            .any(|p| p.matches(&path_str) || p.matches(path.file_name().unwrap_or_default().to_str().unwrap_or("")))
    }

    /// Run the full processing pipeline
    pub async fn run(&self) -> Result<Vec<PathBuf>, ProcessorError> {
        let mp = MultiProgress::new();

        // Phase 1: Discovery & Ingestion
        info!("Phase 1: Discovering notes in {:?}", self.config.input);
        let notes = self.discover_notes()?;
        info!("Found {} notes", notes.len());

        if notes.is_empty() {
            warn!("No notes found to process");
            return Ok(vec![]);
        }

        // Phase 2: Categorization (parallel)
        info!("Phase 2: Categorizing notes...");
        let cat_pb = mp.add(ProgressBar::new(notes.len() as u64));
        cat_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} Categorizing")
                .unwrap()
                .progress_chars("#>-"),
        );

        let categorized = self.categorize_all(notes, cat_pb.clone()).await;
        cat_pb.finish_with_message("Categorization complete");

        let total_segments: usize = categorized.iter().map(|(_, s)| s.len()).sum();
        info!("Categorized into {} segments", total_segments);

        // Dry run: just show the plan
        if self.config.dry_run {
            println!("\n=== DRY RUN: Categorization Plan ===\n");
            for (path, segments) in &categorized {
                println!("  {}", path.display());
                for seg in segments {
                    println!(
                        "   → {}/{} → {:?}",
                        seg.category,
                        seg.subcategory.as_deref().unwrap_or("general"),
                        seg.paths
                    );
                }
            }
            println!("\nTotal: {} notes → {} segments", categorized.len(), total_segments);
            println!("Run without --dry-run to process and write files.");
            return Ok(vec![]);
        }

        // Phase 3: Enhancement (parallel)
        info!("Phase 3: Enhancing segments...");
        let enh_pb = mp.add(ProgressBar::new(total_segments as u64));
        enh_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} Enhancing")
                .unwrap()
                .progress_chars("#>-"),
        );

        let enhanced = self.enhance_all(categorized.clone(), enh_pb.clone()).await;
        enh_pb.finish_with_message("Enhancement complete");
        info!("Enhanced {} segments", enhanced.len());

        // Phase 4: Output
        info!("Phase 4: Writing output files...");
        let grouped = group_by_output_path(enhanced.clone());
        let written = write_outputs(&self.config.output, grouped, self.config.format)?;
        info!("Wrote {} files to {:?}", written.len(), self.config.output);

        // Phase 5: Reorganization pass (optional)
        if self.config.reorganize {
            info!("Phase 5: Running reorganization pass...");
            self.run_reorganization(&written).await?;
        }

        // Phase 6: Cross-referencing (optional)
        if self.config.cross_ref {
            info!("Phase 6: Adding cross-references...");
            self.run_cross_referencing(&written).await?;
        }

        Ok(written)
    }

    /// Discover all notes in the input directory
    fn discover_notes(&self) -> Result<Vec<RawNote>, std::io::Error> {
        let mut notes = Vec::new();

        for entry in WalkDir::new(&self.config.input)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Skip hidden files
            if path
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            // Skip excluded patterns
            if self.is_excluded(path) {
                debug!("Excluded: {}", path.display());
                continue;
            }

            // Read file content
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    if !content.trim().is_empty() {
                        debug!("Discovered: {}", path.display());
                        notes.push(RawNote {
                            path: path.to_path_buf(),
                            content,
                        });
                    }
                }
                Err(e) => {
                    warn!("Could not read {}: {}", path.display(), e);
                }
            }
        }

        Ok(notes)
    }

    /// Categorize all notes in parallel
    async fn categorize_all(
        &self,
        notes: Vec<RawNote>,
        pb: ProgressBar,
    ) -> Vec<(PathBuf, Vec<Segment>)> {
        let client = self.client.clone();
        let semaphore = self.semaphore.clone();

        let results: Vec<_> = stream::iter(notes)
            .map(|note| {
                let client = client.clone();
                let semaphore = semaphore.clone();
                let pb = pb.clone();

                async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    debug!("Categorizing: {}", note.path.display());

                    let result = match categorize_note(&client, &note).await {
                        Ok(segments) => {
                            debug!(
                                "Categorized {} into {} segments",
                                note.path.display(),
                                segments.len()
                            );
                            Some((note.path, segments))
                        }
                        Err(e) => {
                            error!("Failed to categorize {}: {}", note.path.display(), e);
                            None
                        }
                    };
                    pb.inc(1);
                    result
                }
            })
            .buffer_unordered(self.config.parallel)
            .collect()
            .await;

        results.into_iter().flatten().collect()
    }

    /// Enhance all segments in parallel
    async fn enhance_all(
        &self,
        categorized: Vec<(PathBuf, Vec<Segment>)>,
        pb: ProgressBar,
    ) -> Vec<EnhancedSegment> {
        // Flatten into (path, segment) pairs
        let tasks: Vec<_> = categorized
            .into_iter()
            .flat_map(|(path, segments)| {
                segments
                    .into_iter()
                    .map(move |segment| (path.clone(), segment))
            })
            .collect();

        let client = self.client.clone();
        let semaphore = self.semaphore.clone();
        let format = self.config.format;

        let results: Vec<_> = stream::iter(tasks)
            .map(|(path, segment)| {
                let client = client.clone();
                let semaphore = semaphore.clone();
                let pb = pb.clone();

                async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    debug!("Enhancing segment from: {}", path.display());

                    let result = match enhance_segment(&client, &segment, &path, format).await {
                        Ok(enhanced) => Some(enhanced),
                        Err(e) => {
                            error!("Failed to enhance segment from {}: {}", path.display(), e);
                            None
                        }
                    };
                    pb.inc(1);
                    result
                }
            })
            .buffer_unordered(self.config.parallel)
            .collect()
            .await;

        results.into_iter().flatten().collect()
    }

    /// Run reorganization pass to suggest better structure
    async fn run_reorganization(&self, files: &[PathBuf]) -> Result<(), ProcessorError> {
        let file_list: Vec<String> = files
            .iter()
            .map(|p| p.strip_prefix(&self.config.output).unwrap_or(p))
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let system_prompt = r#"You are a file organization expert. Given a list of note files, analyze the structure and suggest improvements.

Consider:
1. Are there files that would be better under a different category?
2. Should any categories be split into subcategories?
3. Are there files that fit better under a new category (e.g., "statistics" as its own category vs under "mathematics")?
4. Are there redundant or overlapping categories?

Return JSON:
{
  "file_moves": [
    {"current_path": "machine_learning/tsne.md", "suggested_path": "statistics/dimensionality_reduction/tsne.md", "reason": "t-SNE is a general statistical technique"}
  ],
  "new_categories": [
    {"category": "statistics", "subcategory": "dimensionality_reduction", "affected_files": ["machine_learning/tsne.md", "machine_learning/pca.md"], "reason": "These are general statistical methods applicable beyond ML"}
  ]
}"#;

        let user_prompt = format!("Current file structure:\n{}", file_list.join("\n"));

        match self.client.chat_json(system_prompt, &user_prompt).await {
            Ok(response) => {
                let json_str = extract_json(&response);
                match serde_json::from_str::<ReorgResponse>(json_str) {
                    Ok(reorg) => {
                        if !reorg.file_moves.is_empty() || !reorg.new_categories.is_empty() {
                            println!("\n=== Reorganization Suggestions ===\n");

                            if !reorg.file_moves.is_empty() {
                                println!("File moves:");
                                for mv in &reorg.file_moves {
                                    println!(
                                        "   {} → {}\n      Reason: {}",
                                        mv.current_path, mv.suggested_path, mv.reason
                                    );
                                }
                            }

                            if !reorg.new_categories.is_empty() {
                                println!("\nNew categories:");
                                for cat in &reorg.new_categories {
                                    println!(
                                        "   {}{}\n      Files: {:?}\n      Reason: {}",
                                        cat.category,
                                        cat.subcategory
                                            .as_ref()
                                            .map(|s| format!("/{}", s))
                                            .unwrap_or_default(),
                                        cat.affected_files,
                                        cat.reason
                                    );
                                }
                            }

                            // Apply the moves
                            for mv in &reorg.file_moves {
                                let src = self.config.output.join(&mv.current_path);
                                let dst = self.config.output.join(&mv.suggested_path);
                                if src.exists() {
                                    if let Some(parent) = dst.parent() {
                                        std::fs::create_dir_all(parent)?;
                                    }
                                    std::fs::rename(&src, &dst)?;
                                    info!("Moved {} → {}", mv.current_path, mv.suggested_path);
                                }
                            }
                        } else {
                            info!("No reorganization needed - structure looks good!");
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse reorganization response: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Reorganization pass failed: {}", e);
            }
        }

        Ok(())
    }

    /// Run cross-referencing to link related notes
    async fn run_cross_referencing(&self, files: &[PathBuf]) -> Result<(), ProcessorError> {
        // Build a map of file path -> content summary
        let mut file_summaries: HashMap<String, String> = HashMap::new();
        for file in files {
            if let Ok(content) = std::fs::read_to_string(file) {
                let rel_path = file
                    .strip_prefix(&self.config.output)
                    .unwrap_or(file)
                    .to_string_lossy()
                    .to_string();
                // Take first 500 chars as summary
                let summary: String = content.chars().take(500).collect();
                file_summaries.insert(rel_path, summary);
            }
        }

        let system_prompt = r#"You are a knowledge linking expert. Given a set of notes with their content summaries, identify meaningful connections between them.

Look for:
1. Notes that reference concepts explained in other notes
2. Notes that build upon knowledge from other notes
3. Related topics that would benefit from cross-linking

Return JSON:
{
  "references": [
    {"from_file": "machine_learning/backprop.md", "to_file": "mathematics/calculus/chain_rule.md", "context": "Backpropagation uses the chain rule"}
  ]
}"#;

        let summaries_str: String = file_summaries
            .iter()
            .map(|(path, summary)| format!("=== {} ===\n{}\n", path, summary))
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = format!("Notes to analyze:\n\n{}", summaries_str);

        match self.client.chat_json(system_prompt, &user_prompt).await {
            Ok(response) => {
                let json_str = extract_json(&response);
                match serde_json::from_str::<CrossRefResponse>(json_str) {
                    Ok(refs) => {
                        if !refs.references.is_empty() {
                            println!("\n=== Cross-References Added ===\n");

                            for xref in &refs.references {
                                // Add reference to the source file
                                let src_path = self.config.output.join(&xref.from_file);
                                if src_path.exists() {
                                    if let Ok(mut content) = std::fs::read_to_string(&src_path) {
                                        let ref_section = format!(
                                            "\n\n---\n\n**See also:** [{}](./{}) - {}\n",
                                            xref.to_file,
                                            relative_path(&xref.from_file, &xref.to_file),
                                            xref.context
                                        );
                                        content.push_str(&ref_section);
                                        std::fs::write(&src_path, content)?;
                                        println!(
                                            "   {} → {} ({})",
                                            xref.from_file, xref.to_file, xref.context
                                        );
                                    }
                                }
                            }
                        } else {
                            info!("No cross-references found");
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse cross-reference response: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Cross-referencing pass failed: {}", e);
            }
        }

        Ok(())
    }
}

/// Calculate relative path from one file to another
fn relative_path(from: &str, to: &str) -> String {
    let from_parts: Vec<&str> = from.split('/').collect();
    let to_parts: Vec<&str> = to.split('/').collect();

    // Find common prefix length
    let common = from_parts
        .iter()
        .zip(to_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Build relative path
    let up_count = from_parts.len() - common - 1;
    let ups = std::iter::repeat("..").take(up_count);
    let downs = to_parts.iter().skip(common);

    ups.chain(downs.map(|s| *s)).collect::<Vec<_>>().join("/")
}

/// Extract JSON from response, handling potential markdown code blocks
fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();

    // Check for markdown code blocks
    if trimmed.starts_with("```") {
        if let Some(start) = trimmed.find('\n') {
            let rest = &trimmed[start + 1..];
            if let Some(end) = rest.rfind("```") {
                return rest[..end].trim();
            }
        }
    }

    trimmed
}
