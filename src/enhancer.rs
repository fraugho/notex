use crate::client::{ClientError, LlmClient};
use crate::types::{EnhancedSegment, OutputFormat, Segment};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnhancementError {
    #[error("LLM client error: {0}")]
    Client(#[from] ClientError),
}

fn get_enhancement_system_prompt(format: OutputFormat) -> String {
    let format_instructions = match format {
        OutputFormat::Markdown => {
            r#"Format: Markdown
- Use proper markdown headers (##, ###) for sections
- Use LaTeX for equations: inline $equation$ or block $$equation$$
- Use bullet points and numbered lists appropriately
- Use code blocks with language hints when showing code
- Use **bold** and *italic* for emphasis"#
        }
        OutputFormat::Plain => {
            r#"Format: Plain text
- Use simple text headers with underlines or caps
- Use ASCII for equations (e.g., x^2 + y^2 = r^2)
- Use simple - or * for bullet points
- Keep formatting minimal but readable"#
        }
    };

    format!(
        r#"You are a note enhancement assistant. Your job is to improve and enrich notes while preserving their meaning.

{}

Enhancement tasks:
1. Fix typos, spelling errors, and grammatical issues
2. For any "?" markers (indicating questions the user had):
   - Provide helpful direction or answer
   - Preserve that it was originally a question using format: "[Q: original question] Your answer/guidance here"
3. Add missing equations where relevant to the topic
4. Suggest 1-2 relevant resources (books, papers, links) if applicable
5. Restructure for clarity while preserving all original information
6. Compress verbose sections while keeping essential details

Rules:
- Do NOT add unrelated information
- Do NOT remove important details
- Do NOT use emojis
- Preserve all links and references from the original
- Keep the same general structure/organization
- Be concise but complete
- Output ONLY the enhanced note content, no meta-commentary"#,
        format_instructions
    )
}

pub async fn enhance_segment(
    client: &LlmClient,
    segment: &Segment,
    original_path: &PathBuf,
    format: OutputFormat,
) -> Result<EnhancedSegment, EnhancementError> {
    let system_prompt = get_enhancement_system_prompt(format);

    let user_prompt = format!(
        "Category: {} ({})\n\nOriginal note segment:\n{}",
        segment.category,
        segment.subcategory.as_deref().unwrap_or("general"),
        segment.content
    );

    let enhanced_content = client.chat(&system_prompt, &user_prompt).await?;

    // Combine primary paths with cross-file paths
    let mut all_paths = segment.paths.clone();
    all_paths.extend(segment.cross_file_to.clone());

    Ok(EnhancedSegment {
        original_path: original_path.clone(),
        content: enhanced_content,
        category: segment.category.clone(),
        subcategory: segment.subcategory.clone(),
        output_paths: all_paths,
    })
}
