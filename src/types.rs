use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Broad categories for notes - LLM can suggest subcategories dynamically
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    // Sciences
    Mathematics,
    Statistics,
    Physics,
    Chemistry,
    Biology,
    ComputerScience,

    // Applied
    MachineLearning,
    Engineering,
    Finance,

    // Humanities
    Philosophy,
    History,
    Literature,
    Languages,

    // Personal
    Journal,
    Ideas,
    Todo,

    // Media
    Books,
    Videos,
    Articles,
    Podcasts,

    // Misc
    Reference,
    Links,
    Uncategorized,

    // Custom category from LLM
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Custom(s) => write!(f, "{}", s),
            other => {
                let s = format!("{:?}", other);
                write!(f, "{}", s.to_lowercase())
            }
        }
    }
}

/// Output format for processed notes
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Plain,
}

/// A raw note loaded from disk
#[derive(Debug, Clone)]
pub struct RawNote {
    pub path: PathBuf,
    pub content: String,
}

/// A segment extracted from a note by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub content: String,
    pub category: Category,
    #[serde(default)]
    pub subcategory: Option<String>,
    pub paths: Vec<String>,
    #[serde(default)]
    pub cross_file_to: Vec<String>,
}

/// Response from the categorization LLM call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizationResponse {
    pub segments: Vec<Segment>,
}

/// An enhanced segment ready for output
#[derive(Debug, Clone)]
pub struct EnhancedSegment {
    pub original_path: PathBuf,
    pub content: String,
    pub category: Category,
    pub subcategory: Option<String>,
    pub output_paths: Vec<String>,
}

/// Suggestion for reorganizing file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorgSuggestion {
    pub current_path: String,
    pub suggested_path: String,
    pub reason: String,
}

/// Suggestion for a new subcategory or category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySuggestion {
    pub category: String,
    pub subcategory: Option<String>,
    pub affected_files: Vec<String>,
    pub reason: String,
}

/// Response from the reorganization LLM call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorgResponse {
    #[serde(default)]
    pub file_moves: Vec<ReorgSuggestion>,
    #[serde(default)]
    pub new_categories: Vec<CategorySuggestion>,
}

/// A cross-reference between notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReference {
    pub from_file: String,
    pub to_file: String,
    pub context: String,
}

/// Response from the cross-reference LLM call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRefResponse {
    pub references: Vec<CrossReference>,
}
