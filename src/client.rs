use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("OpenAI API error: {0}")]
    OpenAI(#[from] async_openai::error::OpenAIError),
    #[error("No response content from LLM")]
    NoContent,
    #[error("Max retries exceeded after {0} attempts")]
    MaxRetries(usize),
}

/// Wrapper around async-openai client with custom base URL support
#[derive(Clone)]
pub struct LlmClient {
    client: Client<OpenAIConfig>,
    model: String,
    max_retries: usize,
}

impl LlmClient {
    /// Create a new LLM client with custom base URL
    pub fn new(base_url: &str, api_key: &str, model: &str, max_retries: usize) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(api_key);

        Self {
            client: Client::with_config(config),
            model: model.to_string(),
            max_retries,
        }
    }

    /// Send a chat completion request with retry logic
    async fn chat_internal(&self, system: &str, user: &str) -> Result<String, ClientError> {
        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user)
                .build()?
                .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()?;

        let response = self.client.chat().create(request).await?;

        response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or(ClientError::NoContent)
    }

    /// Send a chat completion request with automatic retry
    pub async fn chat(&self, system: &str, user: &str) -> Result<String, ClientError> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.chat_internal(system, user).await {
                Ok(response) => {
                    if attempt > 1 {
                        debug!("Succeeded on attempt {}", attempt);
                    }
                    return Ok(response);
                }
                Err(e) => {
                    warn!("Attempt {}/{} failed: {}", attempt, self.max_retries, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        // Exponential backoff: 1s, 2s, 4s, ...
                        let delay = Duration::from_secs(1 << (attempt - 1));
                        debug!("Retrying in {:?}...", delay);
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ClientError::MaxRetries(self.max_retries)))
    }

    /// Send a chat completion request expecting JSON response
    pub async fn chat_json(&self, system: &str, user: &str) -> Result<String, ClientError> {
        let system_with_json = format!(
            "{}\n\nIMPORTANT: Respond with valid JSON only. No markdown code blocks, no explanations outside the JSON.",
            system
        );
        self.chat(&system_with_json, user).await
    }
}
