use anyhow::{anyhow, Result};
use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
        ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest,
    },
    Client as OpenAiClient,
};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for the LLM client
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub requests_per_minute: u32,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::OpenAI,
            model: "gpt-4-turbo".to_string(),
            max_tokens: 500,
            temperature: 0.1,
            requests_per_minute: 10,
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

/// Supported LLM providers
#[derive(Debug, Clone, PartialEq)]
pub enum LlmProvider {
    OpenAI,
    // Anthropic support can be added later
    // Anthropic,
}

/// Response from the LLM with metadata
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub raw_response: String,
    pub model: String,
    pub tokens_used: Option<u32>,
    pub provider: LlmProvider,
}

/// LLM client with rate limiting and retry logic
pub struct LlmClient {
    openai_client: Option<OpenAiClient>,
    rate_limiter: Arc<RateLimiter<governor::state::direct::NotKeyed, governor::clock::DefaultClock>>,
    config: LlmConfig,
}

impl LlmClient {
    /// Create a new LLM client from configuration
    ///
    /// # Arguments
    /// * `config` - LLM configuration
    /// * `api_key` - API key for the LLM provider
    ///
    /// # Returns
    /// A new LLM client ready to make requests
    pub fn new(config: LlmConfig, api_key: String) -> Result<Self> {
        tracing::info!(
            "Initializing LLM client: provider={:?}, model={}, rate_limit={}/min",
            config.provider,
            config.model,
            config.requests_per_minute
        );

        // Initialize provider-specific client
        let openai_client = match config.provider {
            LlmProvider::OpenAI => {
                let client = OpenAiClient::new().with_api_key(api_key);
                Some(client)
            }
        };

        // Initialize rate limiter
        let requests_per_minute = NonZeroU32::new(config.requests_per_minute)
            .ok_or_else(|| anyhow!("requests_per_minute must be > 0"))?;

        let quota = Quota::per_minute(requests_per_minute);
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        tracing::info!("LLM client initialized successfully");

        Ok(Self {
            openai_client,
            rate_limiter,
            config,
        })
    }

    /// Generate a trading signal from a prompt
    ///
    /// This method:
    /// 1. Rate limits the request
    /// 2. Calls the LLM API with retries
    /// 3. Parses and returns the response
    ///
    /// # Arguments
    /// * `prompt` - The formatted prompt to send to the LLM
    ///
    /// # Returns
    /// LLM response with the decision and metadata
    pub async fn generate_signal(&self, prompt: String) -> Result<LlmResponse> {
        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        tracing::debug!("Sending prompt to LLM (length: {} chars)", prompt.len());

        // Call LLM with retries
        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            match self.call_llm(&prompt).await {
                Ok(response) => {
                    tracing::info!(
                        "LLM response received: model={}, tokens={:?}, length={} chars",
                        response.model,
                        response.tokens_used,
                        response.raw_response.len()
                    );
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt + 1 < self.config.max_retries {
                        let backoff_ms = 2_u64.pow(attempt) * 1000; // Exponential backoff
                        tracing::warn!(
                            "LLM call failed (attempt {}/{}), retrying in {}ms: {}",
                            attempt + 1,
                            self.config.max_retries,
                            backoff_ms,
                            last_error.as_ref().unwrap()
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts failed")))
    }

    /// Internal method to call the LLM API
    async fn call_llm(&self, prompt: &str) -> Result<LlmResponse> {
        match self.config.provider {
            LlmProvider::OpenAI => self.call_openai(prompt).await,
        }
    }

    /// Call OpenAI API
    async fn call_openai(&self, prompt: &str) -> Result<LlmResponse> {
        let client = self
            .openai_client
            .as_ref()
            .ok_or_else(|| anyhow!("OpenAI client not initialized"))?;

        // Build request
        let request = CreateChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(prompt.to_string()),
                    name: None,
                },
            )],
            max_tokens: Some(self.config.max_tokens),
            temperature: Some(self.config.temperature),
            ..Default::default()
        };

        // Call API with timeout
        let response = tokio::time::timeout(
            Duration::from_secs(self.config.timeout_seconds),
            client.chat().create(request),
        )
        .await
        .map_err(|_| anyhow!("LLM request timed out after {}s", self.config.timeout_seconds))?
        .map_err(|e| anyhow!("OpenAI API error: {}", e))?;

        // Extract response text
        let response_text = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| anyhow!("Empty response from LLM"))?;

        Ok(LlmResponse {
            raw_response: response_text,
            model: response.model.clone(),
            tokens_used: response.usage.map(|u| u.total_tokens),
            provider: LlmProvider::OpenAI,
        })
    }

    /// Parse the LLM response to extract trading signal
    ///
    /// Looks for keywords: LONG, SHORT, HOLD
    ///
    /// # Returns
    /// Parsed trading decision
    pub fn parse_signal(response: &LlmResponse) -> Result<TradingDecision> {
        let text = response.raw_response.to_uppercase();

        let decision = if text.contains("LONG") && !text.contains("SHORT") {
            SignalAction::Long
        } else if text.contains("SHORT") && !text.contains("LONG") {
            SignalAction::Short
        } else if text.contains("HOLD") {
            SignalAction::Hold
        } else {
            // If ambiguous or unclear, default to HOLD (conservative)
            tracing::warn!("Could not parse clear signal from LLM response, defaulting to HOLD");
            SignalAction::Hold
        };

        Ok(TradingDecision {
            action: decision,
            reasoning: response.raw_response.clone(),
            confidence: None, // Could be extracted from response if LLM provides it
        })
    }
}

/// Trading action from LLM
#[derive(Debug, Clone, PartialEq)]
pub enum SignalAction {
    Long,
    Short,
    Hold,
}

/// Parsed trading decision from LLM
#[derive(Debug, Clone)]
pub struct TradingDecision {
    pub action: SignalAction,
    pub reasoning: String,
    pub confidence: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LlmConfig::default();
        assert_eq!(config.provider, LlmProvider::OpenAI);
        assert_eq!(config.model, "gpt-4-turbo");
        assert_eq!(config.max_tokens, 500);
        assert_eq!(config.temperature, 0.1);
        assert_eq!(config.requests_per_minute, 10);
    }

    #[test]
    fn test_parse_signal_long() {
        let response = LlmResponse {
            raw_response: "Based on the analysis, I recommend LONG position. RSI is oversold.".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: Some(50),
            provider: LlmProvider::OpenAI,
        };

        let decision = LlmClient::parse_signal(&response).unwrap();
        assert_eq!(decision.action, SignalAction::Long);
    }

    #[test]
    fn test_parse_signal_short() {
        let response = LlmResponse {
            raw_response: "Market is overbought, recommend SHORT position.".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: Some(50),
            provider: LlmProvider::OpenAI,
        };

        let decision = LlmClient::parse_signal(&response).unwrap();
        assert_eq!(decision.action, SignalAction::Short);
    }

    #[test]
    fn test_parse_signal_hold() {
        let response = LlmResponse {
            raw_response: "Market is unclear, recommend HOLD.".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: Some(50),
            provider: LlmProvider::OpenAI,
        };

        let decision = LlmClient::parse_signal(&response).unwrap();
        assert_eq!(decision.action, SignalAction::Hold);
    }

    #[test]
    fn test_parse_signal_ambiguous() {
        let response = LlmResponse {
            raw_response: "Could go either way, unclear signal.".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: Some(50),
            provider: LlmProvider::OpenAI,
        };

        let decision = LlmClient::parse_signal(&response).unwrap();
        assert_eq!(decision.action, SignalAction::Hold); // Default to HOLD
    }

    #[test]
    fn test_parse_signal_conflicting() {
        // If both LONG and SHORT appear, default to HOLD
        let response = LlmResponse {
            raw_response: "Could be LONG or SHORT depending on...".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: Some(50),
            provider: LlmProvider::OpenAI,
        };

        let decision = LlmClient::parse_signal(&response).unwrap();
        assert_eq!(decision.action, SignalAction::Hold);
    }
}
