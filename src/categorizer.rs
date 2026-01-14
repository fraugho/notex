use crate::client::{ClientError, LlmClient};
use crate::types::{CategorizationResponse, RawNote, Segment};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CategorizationError {
    #[error("LLM client error: {0}")]
    Client(#[from] ClientError),
    #[error("Failed to parse categorization response: {0}")]
    Parse(#[from] serde_json::Error),
}

const CATEGORIZATION_SYSTEM_PROMPT: &str = r#"You are a note categorization assistant. Given a note, extract distinct segments and categorize each.

Available categories (use these exact values):
- mathematics, statistics, physics, chemistry, biology, computer_science
- machine_learning, engineering, finance
- philosophy, history, literature, languages
- journal, ideas, todo
- books, videos, articles, podcasts
- reference, links, uncategorized

For each segment you identify:
1. Extract the relevant content
2. Assign a category from the list above
3. Optionally add a subcategory for more specific organization (e.g., "topology" for mathematics)
4. Suggest output path(s) using format: category/subcategory.md or category/topic.md
5. If content fits multiple subjects, add cross_file_to paths

Return JSON in this exact format:
{
  "segments": [
    {
      "content": "the extracted content here",
      "category": "mathematics",
      "subcategory": "topology",
      "paths": ["mathematics/topology.md"],
      "cross_file_to": []
    }
  ]
}

Rules:
- Keep segment content meaningful and complete
- Preserve important information, links, and references
- If a note has multiple distinct topics, create multiple segments
- If a note is a single coherent piece, create one segment
- Use lowercase for categories and paths
- Preserve any "?" markers as they indicate questions the user had"#;

pub async fn categorize_note(
    client: &LlmClient,
    note: &RawNote,
) -> Result<Vec<Segment>, CategorizationError> {
    let user_prompt = format!(
        "Original file path: {}\n\nNote content:\n{}",
        note.path.display(),
        note.content
    );

    let response = client
        .chat_json(CATEGORIZATION_SYSTEM_PROMPT, &user_prompt)
        .await?;

    // Try to extract JSON from response (handle potential markdown code blocks)
    let json_str = extract_json(&response);

    let categorization: CategorizationResponse = serde_json::from_str(json_str)?;
    Ok(categorization.segments)
}

/// Extract JSON from response, handling potential markdown code blocks
fn extract_json(response: &str) -> &str {
    let trimmed = response.trim();

    // Check for markdown code blocks
    if trimmed.starts_with("```") {
        // Find the end of the opening fence
        if let Some(start) = trimmed.find('\n') {
            let rest = &trimmed[start + 1..];
            // Find the closing fence
            if let Some(end) = rest.rfind("```") {
                return rest[..end].trim();
            }
        }
    }

    // Return as-is if no code blocks
    trimmed
}
