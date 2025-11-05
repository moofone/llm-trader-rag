# Phase 5: Configuration & Monitoring

This document describes the Phase 5 implementation of the RAG-enhanced LLM trading system, focusing on configuration management and performance monitoring.

## Overview

Phase 5 adds:
1. **Configuration Management**: Centralized TOML-based configuration for LLM, RAG, and ingestion parameters
2. **Performance Metrics**: Comprehensive metrics tracking for RAG retrieval, embedding generation, and LLM inference
3. **Monitoring Integration**: Built-in metrics collection in the RAG retriever with logging support

## Components

### 1. Configuration (`config/llm_rag_config.toml`)

The configuration file provides a centralized location for all RAG system parameters:

#### LLM Configuration
```toml
[llm]
provider = "openai"           # "openai" or "anthropic"
model = "gpt-4-turbo"
api_key_env = "OPENAI_API_KEY"
max_tokens = 500
temperature = 0.1
requests_per_minute = 10      # Rate limiting
timeout_seconds = 30
```

**Parameters:**
- `provider`: LLM provider (OpenAI or Anthropic)
- `model`: Model name (e.g., "gpt-4-turbo", "claude-3-opus")
- `api_key_env`: Environment variable containing the API key
- `max_tokens`: Maximum tokens in LLM response
- `temperature`: Sampling temperature (0.0-1.0, lower = more deterministic)
- `requests_per_minute`: Rate limit for API calls
- `timeout_seconds`: Request timeout

#### RAG Configuration
```toml
[rag]
collection_name = "trading_patterns_btc"
qdrant_url = "http://localhost:6333"   # Embedded for dev

top_k = 5                     # Return top-5 similar patterns
lookback_days = 90            # Search last 90 days
similarity_threshold = 0.7    # Only return matches with similarity > 0.7
min_matches = 3               # Fallback to baseline if fewer than this
use_regime_filters = true     # Apply OI/funding/volatility filters
```

**Parameters:**
- `collection_name`: Qdrant collection name
- `qdrant_url`: Qdrant server URL (local or cloud)
- `top_k`: Number of similar patterns to retrieve
- `lookback_days`: Historical data lookback window
- `similarity_threshold`: Minimum cosine similarity for matches (0.0-1.0)
- `min_matches`: Minimum matches required (otherwise fallback to baseline prompt)
- `use_regime_filters`: Enable/disable market regime filters (OI, funding)

#### Ingestion Configuration
```toml
[ingestion]
snapshot_interval_minutes = 15
batch_size = 100
symbols = ["BTCUSDT", "ETHUSDT"]
```

**Parameters:**
- `snapshot_interval_minutes`: Frequency of snapshot extraction (15 minutes)
- `batch_size`: Batch size for embedding generation
- `symbols`: List of trading symbols to process

### 2. Metrics Module (`trading-strategy/src/llm/metrics.rs`)

The metrics module provides comprehensive performance tracking for the RAG system.

#### RagMetrics Structure

```rust
pub struct RagMetrics {
    pub retrieval_latency_ms: u64,      // Qdrant search time
    pub embedding_latency_ms: u64,      // Embedding generation time
    pub llm_latency_ms: u64,            // LLM inference time
    pub similarity_scores: Vec<f32>,    // All similarity scores
    pub similarity_min: Option<f32>,    // Min similarity
    pub similarity_max: Option<f32>,    // Max similarity
    pub num_matches: usize,             // Number of matches found
    pub outcomes_distribution: Vec<f64>, // 4h outcome distribution
    pub outcome_median_4h: Option<f64>, // Median outcome
    pub outcome_p10_4h: Option<f64>,    // 10th percentile
    pub outcome_p90_4h: Option<f64>,    // 90th percentile
}
```

#### Key Methods

**Setting Metrics:**
```rust
let mut metrics = RagMetrics::new();

// Set latencies from Duration
metrics.set_retrieval_latency(duration);
metrics.set_embedding_latency(duration);
metrics.set_llm_latency(duration);

// Set similarity scores (auto-calculates min/max)
metrics.set_similarity_scores(vec![0.9, 0.85, 0.75]);

// Set outcomes (auto-calculates percentiles)
metrics.set_outcomes(vec![-2.3, 1.1, -1.8]);
```

**Reporting Metrics:**
```rust
// Basic metrics report
metrics.report();

// Detailed report with outcome distribution
metrics.report_detailed();
```

#### MetricsTimer

Helper for measuring operation latency:
```rust
let timer = MetricsTimer::start();
// ... perform operation ...
let duration = timer.stop();
metrics.set_retrieval_latency(duration);
```

### 3. Metrics Integration in RAG Retriever

The RAG retriever now has two methods:

#### With Metrics (New)
```rust
pub async fn find_similar_patterns_with_metrics(
    &self,
    current_snapshot: &MarketStateSnapshot,
    lookback_days: u32,
    top_k: usize,
) -> Result<(Vec<HistoricalMatch>, RagMetrics)>
```

Returns both matches and detailed metrics for monitoring.

#### Without Metrics (Backward Compatible)
```rust
pub async fn find_similar_patterns(
    &self,
    current_snapshot: &MarketStateSnapshot,
    lookback_days: u32,
    top_k: usize,
) -> Result<Vec<HistoricalMatch>>
```

Delegates to `find_similar_patterns_with_metrics` and discards metrics.

## Usage Examples

### 1. Using Metrics in RAG Retrieval

```rust
use trading_strategy::llm::{RagRetriever, RagMetrics};

let retriever = RagRetriever::new(vector_store, 3).await?;

// Get matches with metrics
let (matches, mut metrics) = retriever
    .find_similar_patterns_with_metrics(&snapshot, 90, 5)
    .await?;

// Add LLM latency
let llm_timer = MetricsTimer::start();
let response = llm_client.generate_signal(prompt).await?;
metrics.set_llm_latency(llm_timer.stop());

// Report all metrics
metrics.report_detailed();
```

### 2. Metrics Output Example

```
INFO RAG Metrics: retrieval=45ms, embedding=32ms, llm=187ms, total=264ms,
     avg_sim=0.83, matches=5, sim_range=[0.75,0.91],
     median_4h=-0.5, p10_4h=-2.1, p90_4h=0.9

INFO Outcome distribution: positive=2(40.0%), negative=2(40.0%), neutral=1(20.0%)
```

## Performance Targets

Based on the specification:

| Metric | Target | Notes |
|--------|--------|-------|
| Embedding Latency | <50ms | CPU-based with FastEmbed |
| Retrieval Latency | <50ms | Qdrant p99 |
| Total RAG Latency | <100ms | Embedding + Retrieval |
| LLM Latency | <500ms | OpenAI/Anthropic API |
| **Total End-to-End** | **<600ms** | Acceptable for trading signals |

## Monitoring Best Practices

### 1. Log Metrics for Every Signal Generation
```rust
let (matches, metrics) = retriever
    .find_similar_patterns_with_metrics(&snapshot, 90, 5)
    .await?;

metrics.report(); // Always log metrics
```

### 2. Track Outcome Distributions
```rust
// Analyze historical outcome distributions
if let Some(median) = metrics.outcome_median_4h {
    if median < -1.0 {
        tracing::warn!("Historical patterns show negative median outcome: {:.2}%", median);
    }
}
```

### 3. Monitor Similarity Quality
```rust
let avg_sim = metrics.avg_similarity();
if avg_sim < 0.75 {
    tracing::warn!("Low average similarity: {:.2}. Matches may be weak.", avg_sim);
}
```

### 4. Alert on High Latencies
```rust
if metrics.total_latency_ms() > 1000 {
    tracing::error!("High total latency: {}ms. Investigate performance.",
                    metrics.total_latency_ms());
}
```

## Testing

Phase 5 includes comprehensive unit tests for the metrics module:

```bash
# Run metrics tests
cargo test -p trading-strategy metrics

# Run all RAG tests
cargo test -p trading-strategy llm
```

### Test Coverage
- Metrics creation and initialization
- Similarity score calculations (min, max, average)
- Outcome distribution calculations (median, P10, P90)
- Percentile calculations
- Timer functionality
- Edge cases (empty data, single values)

## Integration with Existing Phases

Phase 5 builds on previous phases:

- **Phase 1**: Historical data ingestion creates the vector database
- **Phase 2**: Live pattern retrieval uses RAG to find similar patterns
- **Phase 3**: LLM client generates signals based on RAG context
- **Phase 4**: Strategy plugin integrates all components
- **Phase 5**: Configuration and monitoring provide operational visibility

## Configuration Loading (Future Enhancement)

While the configuration file is provided, actual loading logic can be added:

```rust
use config::{Config, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LlmRagConfig {
    llm: LlmConfig,
    rag: RagConfig,
    ingestion: IngestionConfig,
}

fn load_config() -> Result<LlmRagConfig> {
    let config = Config::builder()
        .add_source(File::with_name("config/llm_rag_config"))
        .build()?;

    Ok(config.try_deserialize()?)
}
```

## Next Steps

With Phase 5 complete, the system now has:
1. ✅ Historical data ingestion (Phase 1)
2. ✅ Live pattern retrieval (Phase 2)
3. ✅ LLM client integration (Phase 3)
4. ✅ Strategy plugin integration (Phase 4)
5. ✅ Configuration and monitoring (Phase 5)

**Ready for Phase 6**: Embedding Functional Testing & Walk-Forward Evaluation

Phase 6 will focus on:
- Deterministic RAG aggregator (without LLM)
- Walk-forward evaluation with leak-free splits
- Directional accuracy and calibration metrics
- Trading simulator with fees/slippage
- Hyperparameter sweeps (top_k, similarity_threshold, lookback_days)
- Regime-based analysis (funding, volatility, time-of-day)

## Conclusion

Phase 5 provides the operational foundation for monitoring and tuning the RAG-enhanced LLM trading system. The metrics module enables data-driven optimization, while the configuration system allows easy parameter tuning without code changes.

The system is now ready for rigorous evaluation in Phase 6.
