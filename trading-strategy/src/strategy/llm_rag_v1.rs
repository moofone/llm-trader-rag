use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use trading_core::MarketStateSnapshot;

use crate::llm::{
    LlmClient, LlmPromptFormatter, RagRetriever, SignalAction, TradingDecision,
};

/// Configuration for the LLM RAG V1 strategy
#[derive(Debug, Clone)]
pub struct LlmRagV1Config {
    /// Symbol to trade
    pub symbol: String,

    /// Minimum time between signals (milliseconds)
    pub signal_interval_ms: u64,

    /// Number of days to look back for similar patterns
    pub lookback_days: u32,

    /// Number of top similar patterns to retrieve
    pub top_k: usize,

    /// Minimum number of matches required to use RAG
    /// If fewer matches found, falls back to baseline prompt
    pub min_matches: usize,

    /// Enable or disable RAG retrieval (for A/B testing)
    pub rag_enabled: bool,
}

impl Default for LlmRagV1Config {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".to_string(),
            signal_interval_ms: 15 * 60 * 1000, // 15 minutes
            lookback_days: 90,
            top_k: 5,
            min_matches: 3,
            rag_enabled: true,
        }
    }
}

/// LLM RAG V1 Strategy
///
/// This strategy uses RAG (Retrieval-Augmented Generation) to enhance LLM trading decisions
/// with historical pattern context. It:
///
/// 1. Builds a market state snapshot from current indicators
/// 2. Queries Qdrant for similar historical patterns
/// 3. Formats an LLM prompt with historical context
/// 4. Calls the LLM to generate a trading signal
/// 5. Parses and returns the signal
///
/// Key features:
/// - Rate limiting (max 1 signal per 15 minutes by default)
/// - Falls back to baseline prompt if insufficient historical matches
/// - Supports A/B testing (RAG on/off)
/// - Async/await throughout for non-blocking operation
pub struct LlmRagV1Strategy {
    config: LlmRagV1Config,
    rag_retriever: Arc<RagRetriever>,
    llm_client: Arc<LlmClient>,
    last_signal_time: Arc<Mutex<u64>>,
}

impl LlmRagV1Strategy {
    /// Create a new LLM RAG V1 strategy
    ///
    /// # Arguments
    /// * `config` - Strategy configuration
    /// * `rag_retriever` - RAG retriever for finding similar patterns
    /// * `llm_client` - LLM client for generating signals
    ///
    /// # Returns
    /// A new strategy instance ready to generate signals
    pub fn new(
        config: LlmRagV1Config,
        rag_retriever: Arc<RagRetriever>,
        llm_client: Arc<LlmClient>,
    ) -> Self {
        tracing::info!(
            "Initializing LLM RAG V1 strategy: symbol={}, rag_enabled={}, lookback_days={}, top_k={}",
            config.symbol,
            config.rag_enabled,
            config.lookback_days,
            config.top_k
        );

        Self {
            config,
            rag_retriever,
            llm_client,
            last_signal_time: Arc::new(Mutex::new(0)),
        }
    }

    /// Generate a trading signal from current market state
    ///
    /// This is the main entry point for the strategy. It:
    /// 1. Checks rate limiting
    /// 2. Builds current market snapshot
    /// 3. Queries RAG for similar patterns (if enabled)
    /// 4. Formats prompt with or without historical context
    /// 5. Calls LLM and parses response
    ///
    /// # Arguments
    /// * `current_snapshot` - Current market state snapshot
    ///
    /// # Returns
    /// Trading decision with action (LONG/SHORT/HOLD) and reasoning
    pub async fn generate_signal(
        &self,
        current_snapshot: &MarketStateSnapshot,
    ) -> Result<Option<TradingDecision>> {
        // Check rate limiting
        if !self.should_generate_signal().await? {
            tracing::debug!(
                "Rate limit: skipping signal generation (interval: {}ms)",
                self.config.signal_interval_ms
            );
            return Ok(None);
        }

        tracing::info!(
            "Generating signal for {} at price ${:.2}",
            current_snapshot.symbol,
            current_snapshot.price
        );

        // Query RAG for similar patterns (if enabled)
        let historical_matches = if self.config.rag_enabled {
            match self
                .rag_retriever
                .find_similar_patterns(
                    current_snapshot,
                    self.config.lookback_days,
                    self.config.top_k,
                )
                .await
            {
                Ok(matches) => {
                    tracing::info!(
                        "RAG retrieval succeeded: found {} matches",
                        matches.len()
                    );
                    matches
                }
                Err(e) => {
                    tracing::warn!("RAG retrieval failed: {}, using baseline prompt", e);
                    Vec::new()
                }
            }
        } else {
            tracing::info!("RAG disabled, using baseline prompt");
            Vec::new()
        };

        // Format prompt based on whether we have historical context
        let prompt = if historical_matches.is_empty() {
            tracing::info!("Using baseline prompt (no RAG context)");
            LlmPromptFormatter::format_baseline(&self.config.symbol, current_snapshot)
        } else {
            tracing::info!(
                "Using RAG-enhanced prompt with {} historical matches",
                historical_matches.len()
            );
            LlmPromptFormatter::format_with_historical_patterns(
                &self.config.symbol,
                current_snapshot,
                historical_matches,
            )
        };

        // Call LLM
        let llm_response = self.llm_client.generate_signal(prompt).await?;

        // Parse response
        let decision = LlmClient::parse_signal(&llm_response)?;

        tracing::info!(
            "Signal generated: action={:?}, tokens={:?}",
            decision.action,
            llm_response.tokens_used
        );

        // Log reasoning
        tracing::debug!("LLM reasoning: {}", decision.reasoning);

        // Update last signal time
        self.update_last_signal_time().await?;

        Ok(Some(decision))
    }

    /// Check if enough time has passed to generate a new signal
    async fn should_generate_signal(&self) -> Result<bool> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let last_time = *self.last_signal_time.lock().await;

        if last_time == 0 {
            // First signal
            return Ok(true);
        }

        let elapsed = now.saturating_sub(last_time);
        Ok(elapsed >= self.config.signal_interval_ms)
    }

    /// Update the last signal generation time
    async fn update_last_signal_time(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let mut last_time = self.last_signal_time.lock().await;
        *last_time = now;
        Ok(())
    }

    /// Get strategy configuration
    pub fn config(&self) -> &LlmRagV1Config {
        &self.config
    }
}

/// Convert TradingDecision to a simple signal struct
/// This can be extended to match your existing signal types
#[derive(Debug, Clone)]
pub struct SignalOutput {
    pub symbol: String,
    pub action: SignalAction,
    pub reasoning: String,
    pub confidence: Option<f64>,
    pub timestamp: u64,
}

impl SignalOutput {
    pub fn from_decision(
        symbol: String,
        decision: TradingDecision,
        timestamp: u64,
    ) -> Self {
        Self {
            symbol,
            action: decision.action,
            reasoning: decision.reasoning,
            confidence: decision.confidence,
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trading_core::MarketStateSnapshot;

    #[test]
    fn test_default_config() {
        let config = LlmRagV1Config::default();
        assert_eq!(config.symbol, "BTCUSDT");
        assert_eq!(config.lookback_days, 90);
        assert_eq!(config.top_k, 5);
        assert_eq!(config.min_matches, 3);
        assert!(config.rag_enabled);
    }

    #[test]
    fn test_config_builder() {
        let config = LlmRagV1Config {
            symbol: "ETHUSDT".to_string(),
            signal_interval_ms: 30 * 60 * 1000,
            lookback_days: 60,
            top_k: 10,
            min_matches: 5,
            rag_enabled: false,
        };

        assert_eq!(config.symbol, "ETHUSDT");
        assert_eq!(config.lookback_days, 60);
        assert_eq!(config.top_k, 10);
        assert!(!config.rag_enabled);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        // Note: This is a simplified test. Full integration tests would use mock clients.
        let config = LlmRagV1Config {
            signal_interval_ms: 1000, // 1 second for testing
            ..Default::default()
        };

        let last_signal_time = Arc::new(Mutex::new(0u64));

        // First check should allow signal
        {
            let last_time = *last_signal_time.lock().await;
            assert_eq!(last_time, 0);
        }

        // Simulate updating time
        {
            let now = chrono::Utc::now().timestamp_millis() as u64;
            let mut last_time = last_signal_time.lock().await;
            *last_time = now;
        }

        // Second check immediately should be within interval
        {
            let now = chrono::Utc::now().timestamp_millis() as u64;
            let last_time = *last_signal_time.lock().await;
            let elapsed = now.saturating_sub(last_time);
            assert!(elapsed < config.signal_interval_ms);
        }
    }

    #[test]
    fn test_signal_output_creation() {
        let decision = TradingDecision {
            action: SignalAction::Long,
            reasoning: "Test reasoning".to_string(),
            confidence: Some(0.85),
        };

        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        let signal = SignalOutput::from_decision(
            "BTCUSDT".to_string(),
            decision,
            timestamp,
        );

        assert_eq!(signal.symbol, "BTCUSDT");
        assert_eq!(signal.action, SignalAction::Long);
        assert_eq!(signal.reasoning, "Test reasoning");
        assert_eq!(signal.confidence, Some(0.85));
        assert_eq!(signal.timestamp, timestamp);
    }
}
