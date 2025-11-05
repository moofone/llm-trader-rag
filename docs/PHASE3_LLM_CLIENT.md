# Phase 3: LLM Client Implementation

## Overview

Phase 3 implements the in-system LLM client with rate limiting, retry logic, and response parsing. This component integrates with Phase 2 (RAG retrieval) to provide evidence-based trading decisions from language models.

**Status:** ✅ **Complete**

## Key Features

- ✅ Async LLM client with OpenAI support
- ✅ Configurable rate limiting (requests per minute)
- ✅ Exponential backoff retry logic
- ✅ Response parsing for trading signals (LONG/SHORT/HOLD)
- ✅ Request timeout handling
- ✅ Token usage tracking
- ✅ Conservative decision defaults (ambiguous → HOLD)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Trading Strategy                         │
│                                                             │
│  ┌──────────────────┐    ┌──────────────────┐             │
│  │ Phase 2: RAG     │───▶│ Phase 3: LLM     │             │
│  │ Retriever        │    │ Client           │             │
│  │                  │    │                  │             │
│  │ • Find patterns  │    │ • Rate limiting  │             │
│  │ • Format prompts │    │ • Retry logic    │             │
│  │                  │    │ • Parse signals  │             │
│  └──────────────────┘    └──────────────────┘             │
│           │                        │                        │
│           ▼                        ▼                        │
│     Historical Patterns      LLM Decision                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. LlmClient

The main client for interacting with LLM APIs.

**File:** `trading-strategy/src/llm/llm_client.rs`

**Key Methods:**
- `new(config, api_key)` - Initialize client with configuration
- `generate_signal(prompt)` - Call LLM with rate limiting and retries
- `parse_signal(response)` - Extract trading decision from response

### 2. LlmConfig

Configuration for the LLM client.

**Fields:**
- `provider: LlmProvider` - OpenAI (Anthropic can be added later)
- `model: String` - Model name (e.g., "gpt-4-turbo")
- `max_tokens: u32` - Maximum response tokens (default: 500)
- `temperature: f32` - Sampling temperature (default: 0.1)
- `requests_per_minute: u32` - Rate limit (default: 10)
- `timeout_seconds: u64` - Request timeout (default: 30)
- `max_retries: u32` - Retry attempts (default: 3)

### 3. LlmResponse

LLM response with metadata.

**Fields:**
- `raw_response: String` - Full LLM response text
- `model: String` - Model used
- `tokens_used: Option<u32>` - Token count
- `provider: LlmProvider` - API provider

### 4. TradingDecision

Parsed trading signal from LLM.

**Fields:**
- `action: SignalAction` - LONG, SHORT, or HOLD
- `reasoning: String` - LLM's reasoning
- `confidence: Option<f64>` - Confidence level (if available)

## Usage

### Basic Usage

```rust
use trading_strategy::llm::{LlmClient, LlmConfig, LlmProvider};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Configure LLM client
    let config = LlmConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4-turbo".to_string(),
        max_tokens: 500,
        temperature: 0.1,
        requests_per_minute: 10,
        timeout_seconds: 30,
        max_retries: 3,
    };

    // 2. Initialize with API key
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let llm_client = LlmClient::new(config, api_key)?;

    // 3. Generate signal from prompt
    let prompt = "Based on RSI=75, MACD=50, recommend: LONG, SHORT, or HOLD?";
    let response = llm_client.generate_signal(prompt.to_string()).await?;

    // 4. Parse decision
    let decision = LlmClient::parse_signal(&response)?;

    println!("Decision: {:?}", decision.action);
    println!("Reasoning: {}", decision.reasoning);
    println!("Tokens: {:?}", response.tokens_used);

    Ok(())
}
```

### Integration with Phase 2 (RAG)

```rust
use trading_strategy::llm::{
    LlmClient, LlmConfig, LlmProvider,
    RagRetriever, LlmPromptFormatter,
};
use trading_core::MarketStateSnapshot;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize LLM client
    let config = LlmConfig::default();
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let llm_client = LlmClient::new(config, api_key)?;

    // Initialize RAG retriever (Phase 2)
    let rag_retriever = RagRetriever::new(vector_store, 3).await?;

    // Get current market snapshot
    let current_snapshot = MarketStateSnapshot::from_lmdb(
        &lmdb_manager,
        "BTCUSDT",
        chrono::Utc::now().timestamp_millis() as u64,
    )?;

    // Retrieve similar historical patterns
    let historical_matches = rag_retriever
        .find_similar_patterns(&current_snapshot, 90, 5)
        .await?;

    // Format prompt with RAG context
    let prompt = if historical_matches.is_empty() {
        LlmPromptFormatter::format_baseline("BTCUSDT", &current_snapshot)
    } else {
        LlmPromptFormatter::format_with_historical_patterns(
            "BTCUSDT",
            &current_snapshot,
            historical_matches,
        )
    };

    // Generate signal
    let response = llm_client.generate_signal(prompt).await?;
    let decision = LlmClient::parse_signal(&response)?;

    match decision.action {
        SignalAction::Long => println!("🟢 LONG"),
        SignalAction::Short => println!("🔴 SHORT"),
        SignalAction::Hold => println!("⚪ HOLD"),
    }

    println!("Reasoning: {}", decision.reasoning);

    Ok(())
}
```

## Configuration

### Environment Variables

```bash
# Required
export OPENAI_API_KEY="sk-..."

# Optional (can be set in config instead)
export LLM_MODEL="gpt-4-turbo"
export LLM_MAX_TOKENS="500"
export LLM_TEMPERATURE="0.1"
export LLM_RATE_LIMIT="10"
```

### Configuration File

```toml
# config/llm_config.toml

[llm]
provider = "openai"
model = "gpt-4-turbo"
api_key_env = "OPENAI_API_KEY"
max_tokens = 500
temperature = 0.1
requests_per_minute = 10
timeout_seconds = 30
max_retries = 3
```

## Rate Limiting

The client uses the `governor` crate for rate limiting:

- **Default:** 10 requests per minute
- **Behavior:** Blocks until quota is available
- **Use case:** Prevents API rate limit errors

```rust
// Configure custom rate limit
let config = LlmConfig {
    requests_per_minute: 20,  // 20 req/min
    ..Default::default()
};
```

## Retry Logic

Exponential backoff for transient errors:

1. **Attempt 1:** Immediate
2. **Attempt 2:** Wait 1 second
3. **Attempt 3:** Wait 2 seconds
4. **Attempt 4:** Wait 4 seconds (if max_retries=4)

```rust
let config = LlmConfig {
    max_retries: 5,  // More retries for flaky networks
    ..Default::default()
};
```

## Response Parsing

The `parse_signal()` method extracts trading decisions:

### Parsing Logic

1. **LONG:** Response contains "LONG" but not "SHORT"
2. **SHORT:** Response contains "SHORT" but not "LONG"
3. **HOLD:** Response contains "HOLD" or is ambiguous
4. **Default:** If unclear or conflicting → **HOLD** (conservative)

### Examples

```rust
// Clear LONG signal
"Recommend LONG based on bullish indicators."
→ SignalAction::Long

// Clear SHORT signal
"Market is overbought, recommend SHORT."
→ SignalAction::Short

// Clear HOLD signal
"Uncertain market, recommend HOLD."
→ SignalAction::Hold

// Ambiguous (defaults to HOLD)
"Could go either way."
→ SignalAction::Hold

// Conflicting (defaults to HOLD)
"Could be LONG or SHORT depending on..."
→ SignalAction::Hold
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Empty response from LLM` | LLM returned no content | Retry or check prompt |
| `LLM request timed out` | Request exceeded timeout | Increase `timeout_seconds` |
| `OpenAI API error: 429` | Rate limit exceeded | Reduce `requests_per_minute` |
| `OpenAI API error: 401` | Invalid API key | Check `OPENAI_API_KEY` |
| `All retry attempts failed` | Network/API issues | Check connectivity |

### Error Handling Example

```rust
match llm_client.generate_signal(prompt).await {
    Ok(response) => {
        let decision = LlmClient::parse_signal(&response)?;
        // Use decision
    }
    Err(e) if e.to_string().contains("timeout") => {
        tracing::error!("LLM timeout, using fallback strategy");
        // Use baseline strategy
    }
    Err(e) if e.to_string().contains("429") => {
        tracing::error!("Rate limit hit, backing off");
        // Wait and retry later
    }
    Err(e) => {
        tracing::error!("LLM error: {}", e);
        // Use HOLD as safe default
    }
}
```

## Testing

### Unit Tests

```bash
cargo test --package trading-strategy --lib llm_client
```

### Integration Tests

```bash
cargo test --package trading-strategy --test phase3_integration_test
```

### E2E Tests (requires API key)

```bash
export OPENAI_API_KEY="sk-..."
cargo test --package trading-strategy -- --ignored
```

## Performance Metrics

| Metric | Target | Typical |
|--------|--------|---------|
| Request latency (p50) | <5s | 2-3s |
| Request latency (p99) | <30s | 10-15s |
| Rate limit overhead | <100ms | <50ms |
| Retry overhead | <10s | 3-7s |
| Parse latency | <1ms | <1ms |

## Cost Estimation

### OpenAI GPT-4 Turbo Pricing (as of 2025)

- **Input:** $0.01 / 1K tokens
- **Output:** $0.03 / 1K tokens

### Example Costs

**Single Signal:**
- Prompt: ~800 tokens (with RAG context)
- Response: ~150 tokens
- Cost: $0.008 + $0.0045 = **~$0.013 per signal**

**Daily Trading (1 signal every 15 min):**
- Signals per day: 96
- Daily cost: 96 × $0.013 = **~$1.25/day**
- Monthly cost: **~$37.50/month**

### Cost Optimization

1. **Reduce prompt size:** Shorter historical context
2. **Lower max_tokens:** Set to 300 instead of 500
3. **Use GPT-3.5:** ~10x cheaper, but lower quality
4. **Cache similar prompts:** Avoid redundant calls

```rust
// Cost-optimized config
let config = LlmConfig {
    model: "gpt-3.5-turbo".to_string(),  // Cheaper
    max_tokens: 300,                      // Shorter responses
    requests_per_minute: 5,               // Fewer calls
    ..Default::default()
};
```

## Monitoring

### Recommended Metrics

```rust
// Log these for monitoring
tracing::info!(
    "LLM call: model={}, tokens={}, latency={}ms, action={:?}",
    response.model,
    response.tokens_used.unwrap_or(0),
    latency_ms,
    decision.action
);
```

### Key Metrics to Track

1. **Latency:** Request duration (p50, p95, p99)
2. **Tokens:** Usage per request (for cost tracking)
3. **Decisions:** Distribution of LONG/SHORT/HOLD
4. **Errors:** Rate limit hits, timeouts, API errors
5. **Retries:** Number of retry attempts
6. **Cost:** Daily/monthly token costs

## Future Enhancements

### Anthropic Support

```rust
// Future: Add Anthropic Claude support
pub enum LlmProvider {
    OpenAI,
    Anthropic,  // Coming soon
}
```

### Streaming Responses

```rust
// Future: Stream tokens as they arrive
async fn generate_signal_stream(&self, prompt: String)
    -> impl Stream<Item = String>
{
    // Stream implementation
}
```

### Prompt Caching

```rust
// Future: Cache prompts to reduce costs
let cache_key = hash_snapshot(&current_snapshot);
if let Some(cached) = prompt_cache.get(&cache_key) {
    return cached;
}
```

## Related Documentation

- [Phase 1: Data Ingestion](./PHASE1_INGESTION.md)
- [Phase 2: RAG Retrieval](./PHASE2_RAG_RETRIEVAL.md)
- [Phase 4: Strategy Integration](./PHASE4_INTEGRATION.md)
- [RAG Implementation Spec](../spec/LLM_BOT_RAG_IMPLEMENTATION.md)

## Troubleshooting

### Common Issues

**Q: "OpenAI API error: 401"**
- A: Check that `OPENAI_API_KEY` is set correctly

**Q: "LLM request timed out"**
- A: Increase `timeout_seconds` or check network connectivity

**Q: "Rate limit exceeded"**
- A: Reduce `requests_per_minute` or upgrade API tier

**Q: "Response parsing returns HOLD for everything"**
- A: Check prompt format - ensure it asks for clear LONG/SHORT/HOLD decision

**Q: "High latency (>30s)"**
- A: Use faster model (gpt-3.5-turbo) or reduce `max_tokens`

## Summary

Phase 3 provides a production-ready LLM client with:

- ✅ Rate limiting to prevent API errors
- ✅ Retry logic for reliability
- ✅ Conservative parsing (defaults to HOLD)
- ✅ Token tracking for cost monitoring
- ✅ Timeout protection
- ✅ Clean integration with Phase 2 RAG

**Next:** Phase 4 - Strategy Integration (combining all phases into live trading strategy)
