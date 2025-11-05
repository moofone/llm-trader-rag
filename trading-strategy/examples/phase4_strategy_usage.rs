/// Phase 4: LLM RAG V1 Strategy Usage Example
///
/// This example demonstrates how to:
/// 1. Initialize the RAG retriever
/// 2. Initialize the LLM client
/// 3. Create the strategy
/// 4. Generate trading signals
///
/// Note: This is a code example, not a runnable binary without proper setup.

use std::sync::Arc;
use trading_core::MarketStateSnapshot;
use trading_data_services::VectorStore;
use trading_strategy::{
    LlmClient, LlmConfig, LlmProvider, LlmRagV1Config, LlmRagV1Strategy, RagRetriever,
};

/// Example: Initialize and use the LLM RAG V1 strategy
#[allow(dead_code)]
async fn example_strategy_usage() -> anyhow::Result<()> {
    // ═══════════════════════════════════════════════════════════════════
    // Step 1: Initialize Qdrant Vector Store
    // ═══════════════════════════════════════════════════════════════════
    let qdrant_url = "http://localhost:6333";
    let collection_name = "trading_patterns_btc";

    let vector_store = Arc::new(
        VectorStore::new(qdrant_url, collection_name.to_string()).await?,
    );

    // ═══════════════════════════════════════════════════════════════════
    // Step 2: Initialize RAG Retriever
    // ═══════════════════════════════════════════════════════════════════
    let min_matches = 3; // Minimum number of similar patterns required
    let rag_retriever = Arc::new(RagRetriever::new(vector_store, min_matches).await?);

    // ═══════════════════════════════════════════════════════════════════
    // Step 3: Initialize LLM Client
    // ═══════════════════════════════════════════════════════════════════
    let llm_config = LlmConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4-turbo".to_string(),
        max_tokens: 500,
        temperature: 0.1,
        requests_per_minute: 10,
        timeout_seconds: 30,
        max_retries: 3,
    };

    let api_key = std::env::var("OPENAI_API_KEY")
        .expect("OPENAI_API_KEY must be set");

    let llm_client = Arc::new(LlmClient::new(llm_config, api_key)?);

    // ═══════════════════════════════════════════════════════════════════
    // Step 4: Create Strategy
    // ═══════════════════════════════════════════════════════════════════
    let strategy_config = LlmRagV1Config {
        symbol: "BTCUSDT".to_string(),
        signal_interval_ms: 15 * 60 * 1000, // 15 minutes
        lookback_days: 90,
        top_k: 5,
        min_matches: 3,
        rag_enabled: true,
    };

    let strategy = LlmRagV1Strategy::new(
        strategy_config,
        rag_retriever,
        llm_client,
    );

    // ═══════════════════════════════════════════════════════════════════
    // Step 5: Build Current Market Snapshot
    // ═══════════════════════════════════════════════════════════════════
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    let mut snapshot = MarketStateSnapshot::new(
        "BTCUSDT".to_string(),
        timestamp,
        50000.0,
    );

    // Set current indicators (normally from LMDB or live feed)
    snapshot.rsi_7 = 75.0;
    snapshot.rsi_14 = 70.0;
    snapshot.macd = 50.0;
    snapshot.ema_20 = 49800.0;
    snapshot.ema_20_4h = 50500.0;
    snapshot.ema_50_4h = 50000.0;
    snapshot.open_interest_latest = 110000.0;
    snapshot.open_interest_avg_24h = 100000.0;
    snapshot.funding_rate = 0.0001;
    snapshot.price_change_1h = 2.5;
    snapshot.price_change_4h = 5.0;

    // Set time series (last 10 points)
    snapshot.mid_prices = vec![49000.0, 49200.0, 49500.0, 49800.0, 50000.0, 50100.0, 50200.0, 50300.0, 50400.0, 50500.0];
    snapshot.rsi_7_values = vec![50.0, 55.0, 60.0, 65.0, 70.0, 72.0, 74.0, 75.0, 76.0, 77.0];
    snapshot.macd_values = vec![10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 55.0];
    snapshot.ema_20_values = vec![49000.0, 49100.0, 49200.0, 49300.0, 49400.0, 49500.0, 49600.0, 49700.0, 49800.0, 49900.0];

    // ═══════════════════════════════════════════════════════════════════
    // Step 6: Generate Trading Signal
    // ═══════════════════════════════════════════════════════════════════
    println!("Generating trading signal for {} at ${:.2}", snapshot.symbol, snapshot.price);

    match strategy.generate_signal(&snapshot).await? {
        Some(decision) => {
            println!("✓ Signal generated!");
            println!("  Action: {:?}", decision.action);
            println!("  Reasoning: {}", decision.reasoning);
            if let Some(confidence) = decision.confidence {
                println!("  Confidence: {:.2}", confidence);
            }
        }
        None => {
            println!("⚠ Signal skipped (rate limited)");
        }
    }

    Ok(())
}

/// Example: A/B Testing - Compare RAG vs Baseline
#[allow(dead_code)]
async fn example_ab_testing() -> anyhow::Result<()> {
    // Initialize shared components (vector store, llm client)
    // ... (same as above)

    // Create TWO strategies: one with RAG, one without
    let config_with_rag = LlmRagV1Config {
        symbol: "BTCUSDT".to_string(),
        rag_enabled: true,
        ..Default::default()
    };

    let config_without_rag = LlmRagV1Config {
        symbol: "BTCUSDT".to_string(),
        rag_enabled: false,
        ..Default::default()
    };

    // Generate signals from both strategies and compare
    println!("A/B Test: RAG vs Baseline");
    println!("Strategy A: RAG enabled = {}", config_with_rag.rag_enabled);
    println!("Strategy B: RAG enabled = {}", config_without_rag.rag_enabled);

    Ok(())
}

/// Example: Rate Limiting in Action
#[allow(dead_code)]
async fn example_rate_limiting() -> anyhow::Result<()> {
    // Strategy with 5-minute signal interval
    let config = LlmRagV1Config {
        signal_interval_ms: 5 * 60 * 1000, // 5 minutes
        ..Default::default()
    };

    println!("Signal interval: {}ms ({} minutes)",
             config.signal_interval_ms,
             config.signal_interval_ms / 60_000);

    // First signal: will be generated
    // Second signal within 5 min: will be skipped
    // Third signal after 5 min: will be generated

    Ok(())
}

/// Example: Custom Configuration
#[allow(dead_code)]
fn example_custom_config() {
    // Conservative strategy: longer lookback, more matches required
    let conservative_config = LlmRagV1Config {
        symbol: "BTCUSDT".to_string(),
        signal_interval_ms: 30 * 60 * 1000, // 30 minutes
        lookback_days: 180,                  // 6 months
        top_k: 10,                           // More patterns
        min_matches: 7,                      // Higher threshold
        rag_enabled: true,
    };

    println!("Conservative Strategy Config:");
    println!("  Lookback: {} days", conservative_config.lookback_days);
    println!("  Top-K: {}", conservative_config.top_k);
    println!("  Min matches: {}", conservative_config.min_matches);

    // Aggressive strategy: shorter lookback, fewer matches needed
    let aggressive_config = LlmRagV1Config {
        symbol: "BTCUSDT".to_string(),
        signal_interval_ms: 10 * 60 * 1000, // 10 minutes
        lookback_days: 30,                   // 1 month
        top_k: 3,                            // Fewer patterns
        min_matches: 2,                      // Lower threshold
        rag_enabled: true,
    };

    println!("Aggressive Strategy Config:");
    println!("  Lookback: {} days", aggressive_config.lookback_days);
    println!("  Top-K: {}", aggressive_config.top_k);
    println!("  Min matches: {}", aggressive_config.min_matches);
}

fn main() {
    println!("Phase 4 Strategy Usage Examples");
    println!("================================");
    println!();
    println!("This file contains code examples for using the LLM RAG V1 strategy.");
    println!("See the function implementations above for detailed usage patterns.");
    println!();
    println!("Key components:");
    println!("  1. VectorStore (Qdrant) - Historical pattern database");
    println!("  2. RagRetriever - Find similar patterns");
    println!("  3. LlmClient - Generate trading decisions");
    println!("  4. LlmRagV1Strategy - Coordinate everything");
    println!();
    println!("For full integration, see: trading-strategy/tests/phase4_integration_test.rs");
}
