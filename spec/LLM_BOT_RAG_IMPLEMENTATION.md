# LLM Trading Bot - RAG Implementation Plan

**Status:** Ready for implementation
**Language:** Pure Rust
**Architecture:** Qdrant + FastEmbed-rs + Async OpenAI/Anthropic SDK
**Timeline:** 2-3 weeks for MVP

---

## Executive Summary

This document consolidates the RAG (Retrieval-Augmented Generation) strategy for the trading LLM system. It provides historical pattern context to enhance trade signal generation, allowing the LLM to see "what happened the last 5 times the market looked like this" and make evidence-based decisions.

### Why RAG?

**Without RAG:**
```
Current State: RSI=83, MACD=+72, EMA ratio=1.009
LLM Decision: "RSI is overbought, but MACD shows momentum. I'll go LONG."
→ No historical evidence, pure inference
```

**With RAG:**
```
Current State: RSI=83, MACD=+72, EMA ratio=1.009

Historical Matches:
1. 2025-08-15 14:30 - RSI=82.1, MACD=68.4 → Price dropped -2.3% over next 4h
2. 2025-07-22 09:15 - RSI=84.3, MACD=71.2 → Price rose +1.1% over next 4h
3. 2025-06-30 16:45 - RSI=81.8, MACD=69.8 → Price dropped -1.8% over next 4h
Average outcome: -1.0%

LLM Decision: "Despite MACD momentum, 2/3 similar states led to pullbacks.
Overbought RSI signals mean reversion. I'll HOLD or SHORT."
→ Evidence-based decision with empirical support
```

RAG provides **concrete historical evidence** about what typically happens in similar market conditions, not just theoretical knowledge.

---

## Architecture Decision: Pure Rust

### Why Rust (Not Python)?

| Factor | Rust | Python |
|--------|------|--------|
| **Latency** | <50ms embeddings (CPU-based) | <10ms (needs GPU) |
| **Your tolerance** | 500ms | Well covered |
| **GPU needed** | No (yet) | Yes, for speed |
| **Deployment** | Single binary | Service + deps |
| **Codebase fit** | 100% consistent | Adds complexity |
| **Type safety** | Excellent | None |

**Decision:** Start with Rust. If fine-tuning embeddings becomes critical, train in Python + MLX, export to ONNX, load in FastEmbed-rs (keeps Rust architecture).

### Stack

| Component | Choice | Why |
|-----------|--------|-----|
| **Vector DB** | Qdrant | Rust-native, <50ms p99, free tier covers 130K vectors |
| **Embeddings** | FastEmbed-rs + BGE-small-en-v1.5 | Free, local, fast (384 dims) |
| **Indicator Storage** | LMDB (existing) | Already in codebase |
| **LLM Client** | async-openai or Anthropic SDK | Async/await, rate limiting |
| **Language** | 100% Rust | Type-safe, performant, consistent |

### Migration Path (Future - If Needed)

**When to migrate to Python/MLX:**
- If <50ms embedding latency isn't achieved on CPU
- If you want to fine-tune embeddings on trading data
- If you want custom prediction models beyond RAG

**How:**
1. Build Python training pipeline with MLX (Apple Silicon optimized)
2. Fine-tune BGE model on your market patterns
3. Export to ONNX format
4. Load in FastEmbed-rs (maintains Rust architecture)
5. **OR** run Python as microservice (gRPC/HTTP to Rust core)

---

## Phase 1: Historical Data Ingestion & Vector Database

### Overview

Build a vector database of historical market snapshots with their outcomes. This is the "knowledge base" that the LLM queries during live trading.

```
Historical Data (LMDB)
        │
        ▼
Extract Market Snapshots
        │
        ├─ Current state: RSI, MACD, EMA, OI, funding, etc.
        ├─ Derived features: ratios, slopes, positions
        └─ Outcomes: price changes at 1h/4h/24h
        │
        ▼
Convert to Natural Language
        │
        └─ "RSI(7) is 83.6 (extremely overbought), MACD is 72.8..."
        │
        ▼
Generate Embeddings
        │
        └─ FastEmbed-rs: text → 384-dim vector
        │
        ▼
Store in Qdrant
        │
        └─ Vector + metadata (outcomes, indicators, timestamp)
```

### Step 1.1: Data Compatibility with Workflow Manager

**IMPORTANT:** Ensure RAG data extraction uses the same indicator/derivative data format as the workflow-manager mock server.

**Reference:**
- Mock fixture: `workflow-manager/spec/fixtures/snapshots/backend_authoritative.json`
- Mock command: `npm run mock:nofx -- --host 127.0.0.1 --port 7878 --scenario open`
- Exchange: Bybit (confirmed in fixture `meta.exchange`)

**Data Structure Used:**
- `market_data[SYMBOL]`: Price, MACD, RSI7, RSI14, price changes (1h/4h)
- `market_data[SYMBOL].intraday_series`: 10-point time series (mid_prices, EMA, MACD, RSI, volumes, ratios)
- `market_data[SYMBOL].longer_term`: 4h timeframe data (EMA20/50, ATR, volume, MACD, RSI)
- `derivatives[SYMBOL]`: Open interest (latest + 24h avg), funding rate

**These same indicators feed into RAG snapshot extraction from LMDB.**

---

### Step 1.2: Define MarketStateSnapshot Structure

**File:** `trading-core/src/types/market_snapshot.rs`

```rust
use crate::types::{CryptoFuturesSymbol, TimestampMS};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStateSnapshot {
    // ═══════════════════════════════════════════════════
    // IDENTIFICATION
    // ═══════════════════════════════════════════════════
    pub symbol: String,
    pub timestamp: TimestampMS,
    pub price: f64,

    // ═══════════════════════════════════════════════════
    // CURRENT INDICATORS (3m latest)
    // ═══════════════════════════════════════════════════
    pub rsi_7: f64,              // 7-period RSI (Wilder's)
    pub rsi_14: f64,             // 14-period RSI (Wilder's)
    pub macd: f64,               // MACD line only (EMA12 - EMA26)
    pub ema_20: f64,             // 20-period EMA on 3m

    // ═══════════════════════════════════════════════════
    // 3-MINUTE TIME SERIES (Last 10 points = 30 min history)
    // ═══════════════════════════════════════════════════
    pub mid_prices: Vec<f64>,    // Close prices (last 10 × 3m)
    pub ema_20_values: Vec<f64>, // EMA20 series
    pub macd_values: Vec<f64>,   // MACD series
    pub rsi_7_values: Vec<f64>,  // RSI7 series
    pub rsi_14_values: Vec<f64>, // RSI14 series

    // ═══════════════════════════════════════════════════
    // 4-HOUR LONGER-TERM CONTEXT
    // ═══════════════════════════════════════════════════
    pub ema_20_4h: f64,
    pub ema_50_4h: f64,
    pub atr_3_4h: f64,           // Short-term volatility
    pub atr_14_4h: f64,          // Standard volatility
    pub current_volume_4h: f64,  // Latest 4h candle
    pub avg_volume_4h: f64,      // Average of all 4h candles
    pub macd_4h_values: Vec<f64>, // Last 10 × 4h
    pub rsi_14_4h_values: Vec<f64>, // Last 10 × 4h

    // ═══════════════════════════════════════════════════
    // MARKET MICROSTRUCTURE (Futures-specific)
    // ═══════════════════════════════════════════════════
    pub open_interest_latest: f64,   // Current OI
    pub open_interest_avg_24h: f64,  // 24h average OI
    pub funding_rate: f64,           // Current perpetual funding rate (%)
    pub price_change_1h: f64,        // % change (calculated from 3m/1h candles)
    pub price_change_4h: f64,        // % change (calculated from 4h candles)

    // ═══════════════════════════════════════════════════
    // OUTCOMES (Calculated from FUTURE data)
    // ═══════════════════════════════════════════════════
    pub outcome_15m: Option<f64>, // Price % change after 15 minutes
    pub outcome_1h: Option<f64>,  // Price % change after 1 hour
    pub outcome_4h: Option<f64>,  // Price % change after 4 hours
    pub outcome_24h: Option<f64>, // Price % change after 24 hours

    // ═══════════════════════════════════════════════════
    // OUTCOME METADATA
    // ═══════════════════════════════════════════════════
    pub max_drawdown_1h: Option<f64>,   // Worst intra-period drawdown (%)
    pub max_runup_1h: Option<f64>,      // Best intra-period runup (%)
    pub hit_stop_loss: Option<bool>,    // Did price hit -2% stop?
    pub hit_take_profit: Option<bool>,  // Did price hit +3% target?
}

impl MarketStateSnapshot {
    /// Build snapshot from current LMDB indicator data
    pub fn from_lmdb(
        lmdb_manager: &LmdbManager,
        symbol: &str,
        timestamp: TimestampMS,
    ) -> Result<Self> {
        // Query LMDB for all indicators at this timestamp
        let price = lmdb_manager.get_price(symbol, timestamp)?;
        let rsi_7 = lmdb_manager.get_indicator(symbol, timestamp, "rsi_7")?;
        let rsi_14 = lmdb_manager.get_indicator(symbol, timestamp, "rsi_14")?;
        // ... fetch remaining indicators

        // Calculate derived features
        let ema_ratio_20_50 = ema_20 / ema_50;

        // Calculate time series features (need recent history)
        let rsi_7_slope = Self::calculate_slope(
            lmdb_manager.get_indicator_series(symbol, timestamp - 600_000, timestamp, "rsi_7")?
        );

        // Outcomes will be calculated separately (need future data)
        Ok(Self {
            symbol: symbol.to_string(),
            timestamp,
            price,
            rsi_7,
            rsi_14,
            macd,
            ema_20,
            ema_50,
            atr_14,
            ema_ratio_20_50,
            // macd_momentum removed (no signal/histogram in NOFX-aligned set)
            oi_latest,
            oi_avg_24h,
            oi_delta_pct,
            funding_rate,
            volatility_1h,
            volatility_24h,
            volume_current,
            volume_avg_24h,
            volume_ratio,
            rsi_7_slope,
            macd_slope,
            price_momentum_5m,
            price_momentum_1h,
            outcome_15m: None,  // Filled in next step
            outcome_1h: None,
            outcome_4h: None,
            outcome_24h: None,
            max_drawdown_1h: None,
            max_runup_1h: None,
            hit_stop_loss: None,
            hit_take_profit: None,
        })
    }

    /// Calculate outcomes by looking at future price data
    pub fn calculate_outcomes(&mut self, lmdb: &LmdbManager) -> Result<()> {
        let symbol = &self.symbol;
        let base_price = self.price;
        let base_timestamp = self.timestamp;

        // Helper to calculate % change
        let calc_pct_change = |future_price: f64| -> f64 {
            ((future_price - base_price) / base_price) * 100.0
        };

        // 15-minute outcome
        if let Ok(price_15m) = lmdb.get_price(symbol, base_timestamp + 15 * 60_000) {
            self.outcome_15m = Some(calc_pct_change(price_15m));
        }

        // 1-hour outcome
        if let Ok(price_1h) = lmdb.get_price(symbol, base_timestamp + 60 * 60_000) {
            self.outcome_1h = Some(calc_pct_change(price_1h));
        }

        // 4-hour outcome
        if let Ok(price_4h) = lmdb.get_price(symbol, base_timestamp + 4 * 60 * 60_000) {
            self.outcome_4h = Some(calc_pct_change(price_4h));
        }

        // 24-hour outcome
        if let Ok(price_24h) = lmdb.get_price(symbol, base_timestamp + 24 * 60 * 60_000) {
            self.outcome_24h = Some(calc_pct_change(price_24h));
        }

        // Calculate intra-period metrics (1-hour window)
        self.calculate_intraperiod_metrics(lmdb, base_timestamp, 60 * 60_000)?;

        Ok(())
    }

    fn calculate_intraperiod_metrics(
        &mut self,
        lmdb: &LmdbManager,
        start_timestamp: TimestampMS,
        duration_ms: u64,
    ) -> Result<()> {
        let symbol = &self.symbol;
        let base_price = self.price;

        let mut max_runup = 0.0f64;
        let mut max_drawdown = 0.0f64;
        let mut hit_stop = false;
        let mut hit_tp = false;

        const STOP_LOSS_PCT: f64 = -2.0;
        const TAKE_PROFIT_PCT: f64 = 3.0;

        for minutes in 1..=(duration_ms / 60_000) {
            let ts = start_timestamp + minutes * 60_000;

            if let Ok(price) = lmdb.get_price(symbol, ts) {
                let pct_change = ((price - base_price) / base_price) * 100.0;

                max_runup = max_runup.max(pct_change);
                max_drawdown = max_drawdown.min(pct_change);

                if pct_change <= STOP_LOSS_PCT {
                    hit_stop = true;
                }
                if pct_change >= TAKE_PROFIT_PCT {
                    hit_tp = true;
                }
            }
        }

        self.max_runup_1h = Some(max_runup);
        self.max_drawdown_1h = Some(max_drawdown);
        self.hit_stop_loss = Some(hit_stop);
        self.hit_take_profit = Some(hit_tp);

        Ok(())
    }

    fn calculate_slope(values: Vec<f64>) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        // Simple linear regression slope
        let n = values.len() as f64;
        let x_mean = (values.len() - 1) as f64 / 2.0;
        let y_mean = values.iter().sum::<f64>() / n;

        let numerator: f64 = values.iter().enumerate()
            .map(|(i, &y)| (i as f64 - x_mean) * (y - y_mean))
            .sum();

        let denominator: f64 = values.iter().enumerate()
            .map(|(i, _)| (i as f64 - x_mean).powi(2))
            .sum();

        if denominator.abs() < 1e-10 {
            0.0
        } else {
            numerator / denominator
        }
    }
}
```

### Step 1.2: Natural Language Text Conversion

**File:** `trading-data-services/src/rag/snapshot_formatter.rs`

Convert market snapshots into descriptive text for embedding models:

```rust
pub trait SnapshotFormatter {
    fn to_embedding_text(&self) -> String;
    fn to_embedding_text_simple(&self) -> String;
}

impl SnapshotFormatter for MarketStateSnapshot {
    /// Detailed natural language format (more semantic info for embeddings)
    fn to_embedding_text(&self) -> String {
        let mut parts = Vec::new();

        // Trend indicators
        parts.push(format!("RSI(7) is {:.1}, which is {}",
            self.rsi_7, self.interpret_rsi(self.rsi_7)));
        parts.push(format!("RSI(14) is {:.1}", self.rsi_14));

        // MACD
        parts.push(format!("MACD is {:.2}", self.macd));
        let macd_mom = if self.macd_slope > 0.0 { "rising" } else if self.macd_slope < 0.0 { "falling" } else { "flat" };
        parts.push(format!("MACD momentum is {} (slope {:.3})", macd_mom, self.macd_slope));

        // EMA trend
        let trend = if self.ema_ratio_20_50 > 1.005 {
            "strong uptrend"
        } else if self.ema_ratio_20_50 < 0.995 {
            "strong downtrend"
        } else {
            "sideways"
        };
        parts.push(format!("EMA(20)/EMA(50) ratio is {:.4}, indicating {}",
            self.ema_ratio_20_50, trend));

        // (Bollinger removed for NOFX parity)

        // Open Interest
        let oi_sentiment = if self.oi_delta_pct > 5.0 {
            "rising significantly"
        } else if self.oi_delta_pct < -5.0 {
            "dropping significantly"
        } else {
            "stable"
        };
        parts.push(format!("Open interest is {} ({:+.1}% vs 24h average)",
            oi_sentiment, self.oi_delta_pct));

        // Funding
        let funding_sentiment = if self.funding_rate > 0.0005 {
            "highly positive (longs paying shorts)"
        } else if self.funding_rate < -0.0005 {
            "highly negative (shorts paying longs)"
        } else {
            "neutral"
        };
        parts.push(format!("Funding rate is {}", funding_sentiment));

        // Momentum
        if self.rsi_7_slope.abs() > 2.0 {
            let direction = if self.rsi_7_slope > 0.0 { "accelerating up" } else { "accelerating down" };
            parts.push(format!("RSI momentum is {}", direction));
        }

        // Volatility
        let vol_state = if self.volatility_1h > self.volatility_24h * 1.5 {
            "elevated"
        } else {
            "normal"
        };
        parts.push(format!("Volatility is {}", vol_state));

        // Join all parts
        format!("Market state for {}: {}", self.symbol, parts.join(". "))
    }

    /// Simpler numerical format (faster to process)
    fn to_embedding_text_simple(&self) -> String {
        format!(
            "Symbol: {}, RSI(7): {:.1}, RSI(14): {:.1}, MACD: {:.2}, \
             EMA Ratio 20/50: {:.4}, OI Delta: {:+.1}%, Funding: {:.6}, \
             ATR(14): {:.2}, Price Momentum 1h: {:+.2}%",
            self.symbol, self.rsi_7, self.rsi_14, self.macd,
            self.ema_ratio_20_50, self.oi_delta_pct, self.funding_rate,
            self.atr_14, self.price_momentum_1h
        )
    }
}

impl SnapshotFormatter for MarketStateSnapshot {
    fn interpret_rsi(&self, rsi: f64) -> &'static str {
        match rsi {
            r if r >= 80.0 => "extremely overbought",
            r if r >= 70.0 => "overbought",
            r if r >= 60.0 => "bullish territory",
            r if r >= 40.0 => "neutral",
            r if r >= 30.0 => "bearish territory",
            r if r >= 20.0 => "oversold",
            _ => "extremely oversold",
        }
    }
}
```

### Step 1.3: Specify Data Requirements and Timeframes

**CRITICAL:** Define exactly what data and timeframes are needed from Bybit.

#### Note: Alignment with NOFX System

After analyzing the nofx trading system, we're aligning RAG snapshots with proven nofx indicators and timeframes to ensure consistency across the trading ecosystem.

**NOFX uses:**
- **3-minute candles** (not 1-minute) for intraday indicators
- **4-hour candles** for longer-term context
- **Indicators:** EMA(20, 50), MACD(12/26), RSI(7, 14), ATR(3, 14), Volume
- **Does NOT use:** Bollinger Bands, EMA(200), OBV, Stochastic, ADX
- **Data source:** Binance (Bybit equivalent in our case)

**Raw Data Required (from Bybit → LMDB):**

| Timeframe | Purpose | Why This Timeframe |
|-----------|---------|-------------------|
| **3-minute** | Core indicator calculation (Intraday) | NOFX uses 3m for real-time analysis; faster response than 1m, less noise |
| **4-hour** | Longer-term context & trend confirmation | Proven by nofx to balance trend vs noise; standard in derivatives trading |

**Indicators to Extract (Aligned with NOFX):**

**Current state (point-in-time from 3m latest):**
- RSI(7): 7-period Wilder's smoothing
- RSI(14): 14-period Wilder's smoothing
- MACD: EMA12 - EMA26 (only line, no signal/histogram)
- EMA(20): 20-period exponential moving average
- Price: Current bid/ask mid

**Time series data (last ~10 points, 3-minute intervals):**
- Close prices (last 10 × 3m = 30 minutes of history)
- EMA(20) values (last 10 points)
- MACD values (last 10 points)
- RSI(7) values (last 10 points)
- RSI(14) values (last 10 points)

**Longer-term context (4-hour timeframe):**
- EMA(20) on 4h
- EMA(50) on 4h
- ATR(3): 3-period ATR (short-term volatility)
- ATR(14): 14-period ATR (standard volatility)
- Current volume 4h (latest candle)
- Average volume 4h (mean of all 4h candles available)
- MACD 4h (last 10 × 4h = 40 hours of values)
- RSI(14) 4h (last 10 × 4h = 40 hours of values)

**Market microstructure (futures-specific):**
- Price changes: 1-hour % change, 4-hour % change (calculated from candles)
- Open Interest: Latest value + 24h average
- Funding Rate: Current perpetual funding rate (%)

**Outcomes (calculated from future data):**
- Price 15 minutes ahead: % change
- Price 1 hour ahead: % change
- Price 4 hours ahead: % change
- Price 24 hours ahead: % change
- Max runup in 1h: Highest price reached (%)
- Max drawdown in 1h: Lowest price reached (%)
- Hit stop loss at -2%: Boolean
- Hit take profit at +3%: Boolean

**Removed from initial proposal (not used by nofx):**
- ~~Bollinger Bands~~ - Not computed by nofx
- ~~EMA(200)~~ - Not used; 20/50 sufficient
- ~~OBV~~ - Not used
- ~~VWAP, Stochastic, ADX, Ichimoku~~ - Out of scope

**Data Snapshot Interval:**
- **Extraction frequency:** Every 15 minutes (4 snapshots per hour)
- **Backtest period:** 90 days minimum
  - Per symbol: ~8,640 snapshots (90 days × 96 snapshots/day)
  - 3m series: 10 points × 3 min = 30 min of history per snapshot
  - 4h series: 10 points × 4h = 40h of history per snapshot
- **Symbols:** BTCUSDT, ETHUSDT (expandable to SOL, LINK, etc.)

**Storage estimate:**
- 1 symbol, 90 days @ 15-min: ~8.6K snapshots
- 2 symbols: ~17.3K snapshots
- Per snapshot in Qdrant: ~2KB (vector + metadata, compact format)
- Total: ~34MB for 2 symbols (order of tens of MB)

---

### Step 1.4: Historical Snapshot Extractor

**File:** `trading-data-services/src/rag/snapshot_extractor.rs`

Extract snapshots from LMDB for a given time range. **Data comes from Bybit candles/indicators stored in LMDB.**

**Bybit data flow:**
1. Bybit 1‑min candles → ingested into LMDB (`candles_BTCUSDT_1m`, `candles_ETHUSDT_1m`, etc.)
2. Aggregate 1‑min → 3‑min series for intraday indicator computation (NOFX parity)
3. Bybit 4h candles → ingested into LMDB (`candles_BTCUSDT_4h`, etc.)
4. Indicators computed on 3‑minute (intraday) and 4‑hour (context) series → stored in LMDB
5. Snapshots extracted at 15‑minute intervals
6. Future outcomes calculated by looking forward in LMDB candle data

```rust
pub struct HistoricalSnapshotExtractor {
    lmdb_manager: Arc<LmdbManager>,
}

impl HistoricalSnapshotExtractor {
    pub fn new(lmdb_manager: Arc<LmdbManager>) -> Self {
        Self { lmdb_manager }
    }

    /// Extract snapshots for a symbol in a time range
    ///
    /// # Arguments
    /// * `symbol` - Trading symbol (e.g., "BTCUSDT")
    /// * `start_timestamp` - Start time in milliseconds
    /// * `end_timestamp` - End time in milliseconds
    /// * `interval_minutes` - Snapshot frequency (e.g., 15)
    ///
    /// **Note:** This queries LMDB which is populated from Bybit historical data
    pub fn extract_snapshots(
        &self,
        symbol: &str,
        start_timestamp: TimestampMS,
        end_timestamp: TimestampMS,
        interval_minutes: u64,
    ) -> Result<Vec<MarketStateSnapshot>> {
        let mut snapshots = Vec::new();
        let interval_ms = interval_minutes * 60_000;

        let mut current_ts = start_timestamp;

        while current_ts < end_timestamp {
            match MarketStateSnapshot::from_lmdb(&self.lmdb_manager, symbol, current_ts) {
                Ok(mut snapshot) => {
                    // Calculate outcomes (need future data from LMDB)
                    if let Err(e) = snapshot.calculate_outcomes(&self.lmdb_manager) {
                        tracing::warn!("Failed to calculate outcomes for {}: {}", current_ts, e);
                        // Skip snapshots without full outcome data
                    } else {
                        snapshots.push(snapshot);
                    }
                }
                Err(e) => {
                    tracing::debug!("Skipping timestamp {}: {}", current_ts, e);
                }
            }

            current_ts += interval_ms;
        }

        tracing::info!("Extracted {} snapshots for {} from {} to {} (using Bybit data from LMDB)",
            snapshots.len(), symbol, start_timestamp, end_timestamp);

        Ok(snapshots)
    }
}
```

### Step 1.4: Vector Database Setup (Qdrant)

**File:** `trading-data-services/src/rag/vector_store.rs`

```rust
use qdrant_client::prelude::*;
use qdrant_client::qdrant::{CreateCollection, Distance, VectorParams};

pub struct VectorStore {
    client: QdrantClient,
    collection_name: String,
}

impl VectorStore {
    /// Initialize Qdrant client (embedded for dev, cloud for prod)
    pub async fn new(qdrant_url: &str, collection_name: String) -> Result<Self> {
        let client = QdrantClient::from_url(qdrant_url).build()?;

        // Create collection if it doesn't exist
        match client.create_collection(&CreateCollection {
            collection_name: collection_name.clone(),
            vectors_config: Some(VectorParams {
                size: 384,  // BGE-small dimension
                distance: Distance::Cosine as i32,
                ..Default::default()
            }.into()),
            ..Default::default()
        }).await {
            Ok(_) => {
                tracing::info!("Created Qdrant collection: {}", collection_name);
            }
            Err(e) => {
                // Collection might already exist
                tracing::info!("Qdrant collection {} already exists or error: {}", collection_name, e);
            }
        }

        Ok(Self { client, collection_name })
    }

    /// Upload points to Qdrant
    pub async fn upsert_points(
        &self,
        points: Vec<PointStruct>,
    ) -> Result<()> {
        self.client
            .upsert_points_blocking(&self.collection_name, None, points, None)
            .await?;

        Ok(())
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        query_vector: Vec<f32>,
        limit: u64,
        filter: Option<Filter>,
        score_threshold: Option<f32>,
    ) -> Result<Vec<ScoredPoint>> {
        let search_result = self.client
            .search_points(&SearchPoints {
                collection_name: self.collection_name.clone(),
                vector: query_vector,
                filter,
                limit,
                with_payload: Some(true.into()),
                score_threshold,
                ..Default::default()
            })
            .await?;

        Ok(search_result.result)
    }
}

/// Helper to create Qdrant points from snapshots
pub fn snapshot_to_point(
    snapshot: &MarketStateSnapshot,
    embedding: Vec<f32>,
    point_id: u64,
) -> PointStruct {
    PointStruct::new(
        point_id,
        embedding,
        serde_json::json!({
            // Identification
            "symbol": snapshot.symbol,
            "timestamp": snapshot.timestamp,
            "price": snapshot.price,

            // Indicators
            "rsi_7": snapshot.rsi_7,
            "rsi_14": snapshot.rsi_14,
            "macd": snapshot.macd,
            "ema_ratio": snapshot.ema_ratio_20_50,
            // (Bollinger position removed for NOFX parity)

            // Derivatives
            "oi_delta_pct": snapshot.oi_delta_pct,
            "funding_rate": snapshot.funding_rate,

            // Volatility
            "volatility_1h": snapshot.volatility_1h,
            "volatility_24h": snapshot.volatility_24h,
            "volatility_ratio": if snapshot.volatility_24h.abs() > 1e-9 { snapshot.volatility_1h / snapshot.volatility_24h } else { 1.0 },

            // OUTCOMES - THE VALUABLE PART
            "outcome_1h": snapshot.outcome_1h,
            "outcome_4h": snapshot.outcome_4h,
            "outcome_24h": snapshot.outcome_24h,
            "max_runup_1h": snapshot.max_runup_1h,
            "max_drawdown_1h": snapshot.max_drawdown_1h,
            "hit_stop_loss": snapshot.hit_stop_loss,
            "hit_take_profit": snapshot.hit_take_profit,

            // Metadata & provenance
            "schema_version": 1,
            "feature_version": "v1_nofx_3m4h",
            "embedding_model": "bge-small-en-v1.5",
            "embedding_dim": 384,
            "build_id": std::env::var("GIT_SHA").unwrap_or_else(|_| "dev".to_string()),

            // Metadata
            "date": chrono::DateTime::from_timestamp_millis(snapshot.timestamp as i64)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string()),
        }).into(),
    )
}
```

### Step 1.5: Full Ingestion Pipeline

**File:** `trading-data-services/src/rag/ingestion_pipeline.rs`

```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

pub struct HistoricalIngestionPipeline {
    snapshot_extractor: Arc<HistoricalSnapshotExtractor>,
    embedding_model: TextEmbedding,
    vector_store: Arc<VectorStore>,
}

impl HistoricalIngestionPipeline {
    pub async fn new(
        lmdb_manager: Arc<LmdbManager>,
        qdrant_url: &str,
        collection_name: String,
    ) -> Result<Self> {
        // Initialize embedding model (downloads BGE model on first run)
        let embedding_model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(true)
        )?;

        let snapshot_extractor = Arc::new(HistoricalSnapshotExtractor::new(lmdb_manager));
        let vector_store = Arc::new(VectorStore::new(qdrant_url, collection_name).await?);

        Ok(Self {
            snapshot_extractor,
            embedding_model,
            vector_store,
        })
    }

    /// Ingest all historical data for a symbol
    pub async fn ingest_symbol_history(
        &mut self,
        symbol: &str,
        start_timestamp: TimestampMS,
        end_timestamp: TimestampMS,
        snapshot_interval_minutes: u64,
    ) -> Result<IngestStats> {
        let mut stats = IngestStats::default();

        tracing::info!(
            "Starting ingestion for {} from {} to {} (every {} minutes)",
            symbol,
            chrono::DateTime::from_timestamp_millis(start_timestamp as i64)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string()),
            chrono::DateTime::from_timestamp_millis(end_timestamp as i64)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string()),
            snapshot_interval_minutes
        );

        // Step 1: Extract snapshots from LMDB
        let snapshots = self.snapshot_extractor.extract_snapshots(
            symbol,
            start_timestamp,
            end_timestamp,
            snapshot_interval_minutes,
        )?;

        stats.snapshots_created = snapshots.len();
        tracing::info!("Created {} snapshots for {}", snapshots.len(), symbol);

        // Step 2: Generate embeddings in batches
        const BATCH_SIZE: usize = 100;
        let mut all_points = Vec::new();
        let mut point_id = 0u64;

        for batch in snapshots.chunks(BATCH_SIZE) {
            // Convert to text
            let texts: Vec<String> = batch.iter()
                .map(|s| s.to_embedding_text())
                .collect();

            // Generate embeddings (much faster in batch)
            let embeddings = self.embedding_model.embed(texts, None)?;
            stats.embeddings_generated += embeddings.len();

            // Create Qdrant points
            for (snapshot, embedding) in batch.iter().zip(embeddings.iter()) {
                let point = snapshot_to_point(snapshot, embedding.clone(), point_id);
                all_points.push(point);
                point_id += 1;
            }

            tracing::info!("Processed {} embeddings", stats.embeddings_generated);
        }

        // Step 3: Upload to Qdrant
        if !all_points.is_empty() {
            self.vector_store.upsert_points(all_points).await?;
            stats.points_uploaded = point_id as usize;
            tracing::info!("Uploaded {} points to Qdrant", stats.points_uploaded);
        }

        tracing::info!("Ingestion complete for {}: {:?}", symbol, stats);
        Ok(stats)
    }
}

#[derive(Debug, Default)]
pub struct IngestStats {
    pub snapshots_created: usize,
    pub embeddings_generated: usize,
    pub points_uploaded: usize,
}
```

### Step 1.6: Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
# RAG infrastructure
fastembed = "4.1"                    # Local embeddings (ONNX-based)
qdrant-client = "1.12"               # Vector database client
moka = { version = "0.12", features = ["future"] }  # Async caching

# Existing
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
```

---

## Phase 2: Live Pattern Retrieval

### Overview

During signal generation, retrieve similar historical patterns and their outcomes to enrich the LLM prompt with empirical evidence.

```
Current Market State
        │
        ▼
Generate Query Embedding
        │
        └─ Same FastEmbed model as training
        │
        ▼
Search Qdrant
        │
        ├─ Filter by symbol + time range
        └─ Return top-5 most similar vectors
        │
        ▼
Retrieve Historical Matches
        │
        └─ Extract outcomes from payloads
        │
        ▼
Inject into LLM Prompt
        │
        └─ "Similar patterns show avg -1.0% outcome"
```

### Step 2.1: Build RAG Retriever

**File:** `trading-strategy/src/llm/rag_retriever.rs`

```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use qdrant_client::qdrant::{Filter, SearchPoints, Condition, Range};

#[derive(Debug, Clone)]
pub struct HistoricalMatch {
    pub similarity: f32,           // 0.0 to 1.0 (cosine similarity)
    pub timestamp: u64,
    pub date: String,

    // State at that time
    pub rsi_7: f64,
    pub rsi_14: f64,
    pub macd: f64,
    pub ema_ratio: f64,
    pub oi_delta_pct: f64,
    pub funding_rate: f64,

    // What happened next (THE VALUE)
    pub outcome_1h: Option<f64>,
    pub outcome_4h: Option<f64>,
    pub outcome_24h: Option<f64>,
    pub max_runup_1h: Option<f64>,
    pub max_drawdown_1h: Option<f64>,
    pub hit_stop_loss: Option<bool>,
    pub hit_take_profit: Option<bool>,
}

pub struct RagRetriever {
    embedding_model: TextEmbedding,
    vector_store: Arc<VectorStore>,
    min_matches: usize,
}

impl RagRetriever {
    pub async fn new(vector_store: Arc<VectorStore>, min_matches: usize) -> Result<Self> {
        let embedding_model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallENV15)
        )?;

        Ok(Self { embedding_model, vector_store, min_matches })
    }

    pub async fn find_similar_patterns(
        &self,
        current_snapshot: &MarketStateSnapshot,
        lookback_days: u32,
        top_k: usize,
    ) -> Result<Vec<HistoricalMatch>> {
        // 1. Convert current state to embedding
        let query_text = current_snapshot.to_embedding_text();
        let query_embedding = self.embedding_model
            .embed(vec![query_text], None)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Failed to generate embedding"))?;

        // 2. Build filter for recency and symbol
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let lookback_ms = lookback_days as u64 * 86400 * 1000;
        let min_timestamp = now_ms - lookback_ms;

        let mut conditions = vec![
            Condition::field("symbol", current_snapshot.symbol.clone()),
            Condition::field("timestamp")
                .range(Range {
                    gte: Some(min_timestamp as f64),
                    ..Default::default()
                }),
        ];

        // Optional: Filter by market regime (OI delta similar)
        if current_snapshot.oi_delta_pct.abs() > 5.0 {
            let oi_min = current_snapshot.oi_delta_pct - 10.0;
            let oi_max = current_snapshot.oi_delta_pct + 10.0;
            conditions.push(
                Condition::field("oi_delta_pct")
                    .range(Range {
                        gte: Some(oi_min),
                        lte: Some(oi_max),
                        ..Default::default()
                    })
            );
        }

        // Optional: Funding regime (same sign as current)
        if current_snapshot.funding_rate.abs() > 0.0001 {
            if current_snapshot.funding_rate > 0.0 {
                conditions.push(
                    Condition::field("funding_rate")
                        .range(Range { gte: Some(0.0), ..Default::default() })
                );
            } else {
                conditions.push(
                    Condition::field("funding_rate")
                        .range(Range { lte: Some(0.0), ..Default::default() })
                );
            }
        }

        // Optional: Volatility regime (similar 1h/24h ratio)
        if current_snapshot.volatility_24h > 0.0 {
            let ratio = current_snapshot.volatility_1h / current_snapshot.volatility_24h;
            let band = 0.2; // ±20%
            conditions.push(
                Condition::field("volatility_ratio")
                    .range(Range {
                        gte: Some((ratio * (1.0 - band)) as f64),
                        lte: Some((ratio * (1.0 + band)) as f64),
                        ..Default::default()
                    })
            );
        }

        let filter = Filter::must(conditions);

        // 3. Search Qdrant
        let scored_points = self.vector_store.search(
            query_embedding,
            top_k as u64,
            Some(filter),
            Some(0.7),  // Only return good matches
        ).await?;

        // 4. Parse results
        let mut matches = Vec::new();

        for scored_point in scored_points {
            if let Some(payload) = scored_point.payload {
                let match_result = HistoricalMatch {
                    similarity: scored_point.score,
                    timestamp: Self::get_payload_u64(&payload, "timestamp")?,
                    date: Self::get_payload_string(&payload, "date")?,
                    rsi_7: Self::get_payload_f64(&payload, "rsi_7")?,
                    rsi_14: Self::get_payload_f64(&payload, "rsi_14")?,
                    macd: Self::get_payload_f64(&payload, "macd")?,
                    ema_ratio: Self::get_payload_f64(&payload, "ema_ratio")?,
                    oi_delta_pct: Self::get_payload_f64(&payload, "oi_delta_pct")?,
                    funding_rate: Self::get_payload_f64(&payload, "funding_rate")?,
                    outcome_1h: Self::get_payload_f64_opt(&payload, "outcome_1h"),
                    outcome_4h: Self::get_payload_f64_opt(&payload, "outcome_4h"),
                    outcome_24h: Self::get_payload_f64_opt(&payload, "outcome_24h"),
                    max_runup_1h: Self::get_payload_f64_opt(&payload, "max_runup_1h"),
                    max_drawdown_1h: Self::get_payload_f64_opt(&payload, "max_drawdown_1h"),
                    hit_stop_loss: Self::get_payload_bool_opt(&payload, "hit_stop_loss"),
                    hit_take_profit: Self::get_payload_bool_opt(&payload, "hit_take_profit"),
                };
                matches.push(match_result);
            }
        }

        // 5. Enforce minimum match count (fallback to baseline if insufficient)
        if matches.len() < self.min_matches {
            return Ok(Vec::new());
        }

        Ok(matches)
    }

    // Helper methods for payload extraction
    fn get_payload_f64(payload: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<f64> {
        payload.get(key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
    }

    fn get_payload_f64_opt(payload: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<f64> {
        payload.get(key).and_then(|v| v.as_f64())
    }

    fn get_payload_bool_opt(payload: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<bool> {
        payload.get(key).and_then(|v| v.as_bool())
    }

    fn get_payload_string(payload: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<String> {
        payload.get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
    }

    fn get_payload_u64(payload: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<u64> {
        payload.get(key)
            .and_then(|v| v.as_i64())
            .map(|i| i as u64)
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
    }
}
```

### Step 2.2: Prompt Enrichment with RAG Context

**File:** `trading-strategy/src/llm/prompt_formatter.rs`

```rust
pub struct LlmPromptFormatter;

impl LlmPromptFormatter {
    pub fn format_baseline(
        symbol: &str,
        current_snapshot: &MarketStateSnapshot,
    ) -> String {
        let mut prompt = String::new();
        prompt.push_str(&format!("ALL {} DATA\n\n", symbol));
        prompt.push_str(&format!(
            "current_price = {:.1}, current_ema20 = {:.2}, current_macd = {:.2}, current_rsi (7 period) = {:.1}\n\n",
            current_snapshot.price, current_snapshot.ema_20, current_snapshot.macd, current_snapshot.rsi_7
        ));
        prompt.push_str("No historical pattern context available for this cycle.\n\n");
        prompt.push_str("Decide among: LONG, SHORT, or HOLD based on current indicators only.\n");
        prompt
    }

    pub fn format_with_historical_patterns(
        symbol: &str,
        current_snapshot: &MarketStateSnapshot,
        historical_matches: Vec<HistoricalMatch>,
    ) -> String {
        let mut prompt = String::new();

        // === Current State (nof1.ai style) ===
        prompt.push_str(&format!("ALL {} DATA\n\n", symbol));
        prompt.push_str(&format!(
            "current_price = {:.1}, current_ema20 = {:.2}, current_macd = {:.2}, current_rsi (7 period) = {:.1}\n\n",
            current_snapshot.price, current_snapshot.ema_20, current_snapshot.macd, current_snapshot.rsi_7
        ));

        // ... (include current indicators and time series here)

        // === Historical Pattern Analysis ===
        if !historical_matches.is_empty() {
            prompt.push_str("\n\n");
            prompt.push_str("═══════════════════════════════════════════════════════════════════\n");
            prompt.push_str("HISTORICAL PATTERN ANALYSIS - What Happened When Market Looked Like This\n");
            prompt.push_str("═══════════════════════════════════════════════════════════════════\n\n");

            prompt.push_str(&format!(
                "Found {} similar market conditions from the past 90 days:\n\n",
                historical_matches.len()
            ));

            for (i, match_) in historical_matches.iter().enumerate() {
                prompt.push_str(&format!("{}. {} (Similarity: {:.1}%)\n",
                    i + 1, match_.date, match_.similarity * 100.0));

                prompt.push_str(&format!("   Market State: RSI(7)={:.1}, MACD={:.1}, EMA Ratio={:.3}, OI {:+.1}%\n",
                    match_.rsi_7, match_.macd, match_.ema_ratio, match_.oi_delta_pct));

                // The critical part: what happened next
                if let Some(outcome_4h) = match_.outcome_4h {
                    prompt.push_str(&format!("   → 4h later: Price {:+.2}%", outcome_4h));

                    if let (Some(runup), Some(drawdown)) = (match_.max_runup_1h, match_.max_drawdown_1h) {
                        prompt.push_str(&format!(" (max runup: {:+.1}%, max drawdown: {:+.1}%)",
                            runup, drawdown));
                    }

                    if match_.hit_stop_loss == Some(true) {
                        prompt.push_str(" [HIT STOP LOSS]");
                    } else if match_.hit_take_profit == Some(true) {
                        prompt.push_str(" [HIT TAKE PROFIT]");
                    }

                    prompt.push_str("\n");
                }

                prompt.push_str("\n");
            }

            // Summary statistics
            let outcomes_4h: Vec<f64> = historical_matches.iter()
                .filter_map(|m| m.outcome_4h)
                .collect();

            if !outcomes_4h.is_empty() {
                let mut sorted = outcomes_4h.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let len = sorted.len();
                let p = |q: f64| -> f64 {
                    let idx = ((len as f64 - 1.0) * q).round() as usize;
                    sorted[idx]
                };
                let avg = sorted.iter().sum::<f64>() / len as f64;
                let median = p(0.5);
                let p10 = p(0.10);
                let p90 = p(0.90);
                let positive_count = outcomes_4h.iter().filter(|&&x| x > 0.0).count();
                let negative_count = outcomes_4h.iter().filter(|&&x| x < 0.0).count();

                let stop_hits = historical_matches.iter()
                    .filter(|m| m.hit_stop_loss == Some(true))
                    .count();

                let tp_hits = historical_matches.iter()
                    .filter(|m| m.hit_take_profit == Some(true))
                    .count();

                prompt.push_str("Summary of Historical Outcomes:\n");
                prompt.push_str(&format!("  Average 4h price change: {:+.2}%\n", avg));
                prompt.push_str(&format!("  Median / P10 / P90 (4h): {:+.2}% / {:+.2}% / {:+.2}%\n", median, p10, p90));
                prompt.push_str(&format!("  Positive outcomes: {}/{} ({:.0}%)\n",
                    positive_count, outcomes_4h.len(),
                    (positive_count as f64 / outcomes_4h.len() as f64) * 100.0));
                prompt.push_str(&format!("  Negative outcomes: {}/{} ({:.0}%)\n",
                    negative_count, outcomes_4h.len(),
                    (negative_count as f64 / outcomes_4h.len() as f64) * 100.0));
                prompt.push_str(&format!("  Hit stop loss: {}, Hit take profit: {}\n",
                    stop_hits, tp_hits));

                // Similarity range
                let (sim_min, sim_max) = historical_matches
                    .iter()
                    .fold((1.0_f32, 0.0_f32), |(mn, mx), m| (mn.min(m.similarity), mx.max(m.similarity)));
                prompt.push_str(&format!("  Similarity range: {:.0}% – {:.0}%\n", sim_min * 100.0, sim_max * 100.0));
            }
        } else {
            prompt.push_str("\n\n[No similar historical patterns found - making decision based on current data only]\n");
        }

        // === Decision Prompt ===
        prompt.push_str("\n\n");
        prompt.push_str("Based on the current market state AND the historical pattern analysis, should I:\n");
        prompt.push_str("A) Enter LONG position\n");
        prompt.push_str("B) Enter SHORT position\n");
        prompt.push_str("C) HOLD (no position)\n\n");
        prompt.push_str("Consider that historical outcomes provide empirical evidence about what typically ");
        prompt.push_str("happens in similar market conditions. Weight this evidence appropriately in your decision.\n\n");
        prompt.push_str("Provide your decision with reasoning (2-3 sentences).\n");

        prompt
    }
}
```

---

## Phase 3: In-System LLM Client

**File:** `trading-strategy/src/llm/llm_client.rs`

Implement async LLM client with rate limiting and retries:

```rust
use async_openai::Client as OpenAiClient;
use async_openai::types::{CreateChatCompletionRequest, ChatCompletionRequestMessage, Role};
use governor::RateLimiter;

pub struct LlmClient {
    openai_client: OpenAiClient,
    rate_limiter: RateLimiter,
    model: String,
    max_tokens: u16,
    temperature: f32,
}

impl LlmClient {
    pub fn new(api_key: String, requests_per_minute: u32) -> Self {
        Self {
            openai_client: OpenAiClient::new().with_api_key(api_key),
            rate_limiter: RateLimiter::direct(governor::Quota::per_minute(requests_per_minute)),
            model: "gpt-4-turbo".to_string(),
            max_tokens: 500,
            temperature: 0.1,
        }
    }

    pub async fn generate_signal(&self, prompt: String) -> Result<LlmResponse> {
        // Rate limiting
        self.rate_limiter.until_ready().await;

        // Call LLM with retries
        let response = self.openai_client
            .chat()
            .create(CreateChatCompletionRequest {
                model: self.model.clone(),
                messages: vec![
                    ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessage {
                            content: prompt,
                            ..Default::default()
                        }
                    ),
                ],
                max_tokens: Some(self.max_tokens as u32),
                temperature: Some(self.temperature),
                ..Default::default()
            })
            .await?;

        let response_text = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| anyhow!("Empty response from LLM"))?;

        Ok(LlmResponse {
            raw_response: response_text,
            model: self.model.clone(),
            tokens_used: response.usage.map(|u| u.total_tokens as u32),
        })
    }
}

pub struct LlmResponse {
    pub raw_response: String,
    pub model: String,
    pub tokens_used: Option<u32>,
}
```

---

## Phase 4: Integration as Strategy Plugin

**File:** `trading-strategy/src/strategy/strategies/llm_rag_v1.rs`

Create a strategy that uses RAG-enhanced LLM signals:

```rust
pub struct LlmRagV1Strategy {
    symbol: CryptoFuturesSymbol,
    lmdb_manager: Arc<LmdbManager>,
    rag_retriever: Arc<RagRetriever>,
    llm_client: Arc<LlmClient>,
    last_signal_time: Arc<Mutex<u64>>,
}

impl LlmRagV1Strategy {
    /// Async variant to avoid blocking the Tokio runtime
    pub async fn on_indicator_event_async(&mut self, event: &TimeSeriesEvent<IndicatorOutputKind>) -> Result<Option<SignalOrder>> {
        // Rate limit: max 1 signal per 15 minutes
        let now = chrono::Utc::now().timestamp_millis() as u64;
        {
            let mut last_time = self.last_signal_time.blocking_lock();
            if now - *last_time < 15 * 60_000 {
                return Ok(None);  // Too soon
            }
            *last_time = now;
        }

        // Build current snapshot
        let snapshot = self.build_snapshot()?;

        // Query RAG for similar patterns
        let patterns = self.rag_retriever.find_similar_patterns(&snapshot, 90, 5).await?;

        // If not enough matches, fall back to baseline prompt
        let prompt = if patterns.is_empty() {
            LlmPromptFormatter::format_baseline(&self.symbol.to_string(), &snapshot)
        } else {
            LlmPromptFormatter::format_with_historical_patterns(
                &self.symbol.to_string(),
                &snapshot,
                patterns,
            )
        };

        // Call LLM
        let response = self.llm_client.generate_signal(prompt).await?;

        // Parse response and emit signal
        let signal = self.parse_llm_response(&response)?;
        Ok(Some(signal))
    }

    fn build_snapshot(&self) -> Result<MarketStateSnapshot> {
        // Extract current state from indicators and LMDB
        MarketStateSnapshot::from_lmdb(&self.lmdb_manager, &self.symbol.to_string(), chrono::Utc::now().timestamp_millis() as u64)
    }

    fn parse_llm_response(&self, response: &LlmResponse) -> Result<SignalOrder> {
        // Parse action, confidence, position size, stops/targets
        // ... implementation details
        Ok(SignalOrder { /* ... */ })
    }
}
```

---

## Phase 5: Configuration & Monitoring

### Configuration

**File:** `config/llm_rag_config.toml`

```toml
[llm]
# LLM provider configuration
provider = "openai"           # "openai" or "anthropic"
model = "gpt-4-turbo"
api_key_env = "OPENAI_API_KEY"
max_tokens = 500
temperature = 0.1
requests_per_minute = 10      # Rate limiting
timeout_seconds = 30

[rag]
# Vector database
collection_name = "trading_patterns_btc"
qdrant_url = "http://localhost:6333"   # Embedded for dev
# OR: "https://your-cluster.aws.cloud.qdrant.io" for cloud

# Retrieval parameters
top_k = 5                     # Return top-5 similar patterns
lookback_days = 90            # Search last 90 days
similarity_threshold = 0.7    # Only return matches with similarity > 0.7
min_matches = 3               # Fallback to baseline if fewer than this
use_regime_filters = true     # Apply OI/funding/volatility filters
# Optional ANN tuning
# ef_search = 128

[ingestion]
# Batch processing for efficiency
snapshot_interval_minutes = 15
batch_size = 100
symbols = ["BTCUSDT", "ETHUSDT"]
```

### Monitoring

**File:** `trading-strategy/src/llm/metrics.rs`

Track key metrics:

```rust
pub struct RagMetrics {
    pub retrieval_latency_ms: u64,
    pub embedding_latency_ms: u64,
    pub llm_latency_ms: u64,
    pub similarity_scores: Vec<f32>,
    pub similarity_min: Option<f32>,
    pub similarity_max: Option<f32>,
    pub num_matches: usize,
    pub outcomes_distribution: Vec<f64>,
    pub outcome_median_4h: Option<f64>,
    pub outcome_p10_4h: Option<f64>,
    pub outcome_p90_4h: Option<f64>,
}

impl RagMetrics {
    pub fn report(&self) {
        let avg_sim = if self.similarity_scores.is_empty() { 0.0 } else { self.similarity_scores.iter().sum::<f32>() / self.similarity_scores.len() as f32 };
        tracing::info!(
            "RAG Metrics: retrieval={}ms, embedding={}ms, llm={}ms, avg_sim={:.2}, matches={}, sim_range=[{:?},{:?}], median_4h={:?}, p10_4h={:?}, p90_4h={:?}",
            self.retrieval_latency_ms,
            self.embedding_latency_ms,
            self.llm_latency_ms,
            avg_sim,
            self.num_matches,
            self.similarity_min,
            self.similarity_max,
            self.outcome_median_4h,
            self.outcome_p10_4h,
            self.outcome_p90_4h,
        );
    }
}
```

---

## Example: End-to-End RAG Flow

### Ingestion (One-time)

```bash
# 1. Extract 90 days of BTCUSDT history from LMDB
# 2. Create ~8.6K snapshots per symbol (~17K for BTC+ETH)
# 3. Calculate outcomes for each (price changes at 1h/4h/24h)
# 4. Convert to natural language text
# 5. Generate embeddings (FastEmbed, batched)
# 6. Upload to Qdrant (with metadata)
```

### Live Trading

```
Current time: 2025-11-05 14:30

1. Build snapshot from current LMDB state
   RSI(7)=83.6, MACD=72.8, EMA Ratio=1.009, OI Delta=+4.2%

2. Generate embedding (same FastEmbed model)

3. Query Qdrant for top-5 similar patterns
   Filter: symbol=BTCUSDT, timestamp >= 90 days ago

4. Retrieve results with outcomes:
   - Pattern 1 (89% similar): -2.3% at 4h (hit stop loss)
   - Pattern 2 (87% similar): +1.1% at 4h (hit take profit)
   - Pattern 3 (85% similar): -1.8% at 4h
   - Pattern 4 (82% similar): -0.5% at 4h
   - Pattern 5 (81% similar): +0.9% at 4h

   Average 4h outcome: -0.51% (60% hit stops, 20% hit TP)

5. Format LLM prompt:
   [Current state] + [Historical patterns] + [Decision prompt]

6. Call LLM (Anthropic/OpenAI)

7. LLM response:
   "Despite MACD bullish signal, 3/5 similar patterns led to losses.
    Overbought RSI (83.6) signals mean reversion. Recommend HOLD."

8. Parse response → SignalOrder

9. Execute trade or stay in position
```

---

## Testing & Backtesting

### Unit Tests

Test individual components:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_snapshot_creation() {
        // Test MarketStateSnapshot creation and outcome calculation
    }

    #[test]
    fn test_embedding_generation() {
        // Test text conversion and embedding model
    }

    #[tokio::test]
    async fn test_rag_retrieval() {
        // Test pattern matching with mock Qdrant
    }
}
```

### Backtesting

Log RAG decisions for later analysis:

```rust
pub struct RagBacktestLog {
    pub timestamp: u64,
    pub snapshot: MarketStateSnapshot,
    pub patterns: Vec<HistoricalMatch>,
    pub prompt: String,
    pub llm_response: String,
    pub signal: SignalOrder,
    pub actual_outcome_4h: Option<f64>,
}

// Save to JSONL for analysis
// Replay mode: load from JSONL instead of calling LLM
```

---

## Phase 6: Embedding Functional Testing & Walk-Forward Evaluation

### Goals
- Verify that retrieval-augmented evidence correlates with future returns and improves decisions vs a no‑RAG baseline.
- Detect leakage, drift, and regime sensitivity; calibrate thresholds for reliable deployment.

### Datasets & Splits
- Chronological, leak‑free splits.
  - Build (index) window: T0..T1 (used to ingest vectors and metadata only).
  - Evaluation window: T1..T2 (walk‑forward). At each timestamp t in T1..T2, only data ≤ t is visible.
- Walk‑forward schedule: rolling origin with fixed lookback (e.g., 90 days) and a step of 15 minutes.
- Symbols: start with BTCUSDT, ETHUSDT; extend after stability.

### Evaluation Methods
- Mode A — Deterministic RAG Aggregator (no LLM):
  - Query top‑k matches → aggregate `outcome_15m/1h/4h` (mean, median, P10/P90) and similarity stats.
  - Decision rule (configurable):
    - Long if mean_4h ≥ +X bp AND P50 ≥ 0 AND similarity_mean ≥ S.
    - Short if mean_4h ≤ −X bp AND P50 ≤ 0 AND similarity_mean ≥ S.
    - Otherwise HOLD.
  - Pros: isolates embedding + retrieval quality without LLM variance.
- Mode B — LLM with/without RAG:
  - Use full prompt pipeline; compare decisions with RAG vs baseline prompts.
  - Captures end‑to‑end effect including language model behavior.

### Metrics
- Directional accuracy at 15m/1h/4h (decision vs sign of realized return).
- Calibration: reliability curves of predicted means/quantiles vs realized; Brier score for up/down at horizon.
- Trading metrics in a simple simulator: win rate, avg win/loss, Sharpe, max drawdown, SL/TP hit rates; include fees/slippage.
- Retrieval quality: similarity mean/min/max, effective‑k, fraction above `similarity_threshold`.
- Uplift vs baselines: no‑RAG prompt, RSI7 threshold, MACD sign, random.
- Robustness by regime: funding sign, volatility_ratio terciles, hour‑of‑day, weekday/weekend.

### Controls & Negative Tests
- Shuffle outcomes in payloads → performance should revert to baseline.
- Cross‑symbol retrieval (mismatch) must be filtered out.
- Random embeddings (noise) → near‑zero similarity and no consistent signal.
- Ablations: remove funding filter / volatility filter / reduce k to measure contribution.

### Hyperparameters to Sweep
- `top_k` ∈ {5, 10, 20}; `similarity_threshold` ∈ {0.65, 0.7, 0.75}.
- Lookback days ∈ {30, 60, 90, 120}.
- Regime filters on/off; volatility_ratio band ∈ {±10%, ±20%, ±30%}.
- Decision thresholds X (bp) for Long/Short; `min_matches` ∈ {3, 5}.

### Implementation Plan (Rust)
- Module: `trading-strategy/src/llm/rag_eval.rs`.
- CLI: `cargo run --bin rag-eval -- --symbol BTCUSDT --start 2025-05-01 --end 2025-07-31 --mode aggregator --top-k 10 --sim-threshold 0.7 --lookback-days 90 --fees-bps 2`.
- Uses same LMDB/Qdrant; enforces leak‑free evaluation by limiting search to points with `timestamp ≤ t`.
- Outputs `reports/rag_eval/{symbol}/{run_id}/metrics.json` and CSVs: trades, step‑level retrieval stats, calibration bins.

#### Pseudo-code
```rust
for t in walk_forward_times(start=T1, end=T2, step=15m) {
    let snap = Snapshot::from_lmdb_at(t);
    let matches = retriever.search_with_max_ts(&snap, lookback_days, top_k, t)?;
    let decision = if mode == Aggregator { rule_based(matches) } else { llm_decision(snap, matches) };
    let realized = realize_outcome(symbol, t, horizon=H);
    metrics.update(decision, realized, matches.stats());
    simulator.apply(decision, price_at(t), fees);
}
metrics.finalize();
```

### Reports & Visuals
- Reliability plots (predicted mean vs realized; quantile calibration).
- Lift charts vs baseline; confusion matrices by horizon.
- Similarity histograms and effective‑k over time.
- Regime breakdown tables (funding sign, volatility terciles).

### CI Gate & Regression Alarms
- Nightly 7-day walk-forward must not regress >X% on Sharpe/uplift.
- Alert if fraction of results above `similarity_threshold` drops by >Y% or leakage is detected.

### Data Integrity & Freshness Guardrails
- **Single source of truth:** All RAG snapshots must originate from the same LMDB schema version as the workflow-manager fixtures; fail fast if `schema_version`, `feature_version`, or exchange metadata diverge.
- **Freshness gating:** Enforce `max_age_ms` on every snapshot retrieved at runtime; surface metrics when ingestion lag exceeds 2× interval cadence and short-circuit retrieval if data is stale.
- **Time-series invariants:** Add property-based tests (via `proptest`) to validate monotonic timestamps, strictly positive volumes, bounded RSI/MACD ranges, and continuity of 3m/4h windows across random symbol/date permutations.
- **Outcome sanity checks:** TDD the outcome calculators with golden fixtures and property tests that perturb price paths to guarantee drawdowns/runups remain within theoretical maxima for the simulated horizon.
- **Null/NaN hygiene:** Reject any snapshot containing NaN/Inf/None values; add integration tests that load corrupted LMDB fixtures to ensure the ingestion path refuses them.
- **Observability:** Emit `snapshot_freshness_seconds`, `missing_window_count`, and `data_rejection_count` metrics; fail deployment checks if any exceed predefined SLOs.

### Exit Criteria (Sprint)
- Statistically significant uplift vs baseline on the 4h horizon (bootstrap 95% CI not including zero uplift).
- Stable calibration (reliability slope 0.9–1.1) and acceptable drawdowns in the simulator.
- No leakage; retrieval distributions stable week-over-week.

### Sprint Roadmap (TDD + Data Integrity Driven)
**Sprint 1 (Days 1–4) – Data Foundations & Freshness**
- Lock historical datasets, record config digests, and wire ingestion smoke tests that fail on schema drift or missing series.
- Implement snapshot extractor + outcome calculator behind feature flags; drive development with unit tests that cover error paths before wiring LMDB.
- Add property-based tests in `trading-core` ensuring randomly generated time-series (with injected gaps/outliers) are either repaired or rejected deterministically.
- Build freshness monitors (`snapshot_freshness_seconds`, `missing_window_count`) and gate runtime retrieval when lag SLOs are violated.

**Sprint 2 (Days 5–8) – Retrieval & Prompt Enrichment**
- Deliver Qdrant collection management, similarity search API, and prompt formatter with deterministic fixtures for BTC/ETH across multiple regimes.
- Practice TDD by writing failing tests for edge cases (duplicate vectors, stale embeddings, symbol mismatches) before implementing retrieval logic.
- Create property-based tests that fuzz embedding vectors/top-k parameters to ensure ranking monotonicity and enforce symbol partitioning.
- Integrate regression tests that replay historical days and assert no stale data crosses the `max_age_ms` threshold in enrichment payloads.

**Sprint 3 (Days 9–12) – Decisioning, Simulation & CI Guardrails**
- Implement Aggregator + LLM decision modes, hook them into walk-forward simulator, and expose guardrail thresholds in config.
- Write scenario-driven tests first (bull/bear/sideways fixtures) covering decision outputs, followed by property-based tests checking calibration monotonicity across randomized outcome distributions.
- Expand CI pipeline to run nightly walk-forward, bootstrap confidence intervals, and enforce freshness/data-integrity alerts as deployment blockers.
- Harden documentation/playbooks: capture testing matrices, data rejection procedures, and incident runbooks for stale-data detection.

---

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Embedding generation | <50ms | Expected (CPU-based) |
| Vector search | <100ms | Expected (Qdrant) |
| LLM call | <30s | Expected |
| End-to-end signal | <500ms | Expected |
| Similarity scores | >0.7 | TBD (measure after MVP) |
| Cost per signal | <$0.10 | TBD (LLM + infra) |

---

## Timeline

| Phase | Duration | Key Deliverables |
|-------|----------|------------------|
| **1: Ingestion** | 3-5 days | Snapshot extractor, FastEmbed, Qdrant setup |
| **2: Retrieval** | 2-3 days | RAG retriever, prompt enrichment |
| **3: LLM Client** | 3-4 days | Async client, rate limiting, parsing |
| **4: Integration** | 3-5 days | Strategy plugin, backtesting, guardrails |
| **5: Deployment** | 2-3 days | Config, monitoring, testing |
| **Total** | 13-20 days | **2-3 weeks for MVP** |

---

## Success Criteria

### MVP Success Metrics

1. ✅ Ingest 90 days of BTCUSDT → ~8.6K snapshots (15‑min cadence)
2. ✅ Generate embeddings in <50ms per snapshot
3. ✅ Retrieve top-5 patterns in <100ms
4. ✅ End-to-end signal generation in <500ms
5. ✅ Enrich LLM prompts with historical outcomes
6. ✅ LLM decisions reference historical patterns
7. ✅ Backtest shows measurable improvement vs no-RAG baseline

### Quality Metrics

- RAG similarity scores: >0.7 for top matches
- Historical outcome consistency: similar patterns have correlated outcomes
- Win rate improvement: +5-10% vs baseline (to be measured)
- Cost efficiency: <$0.10 per signal (LLM + infrastructure)

---

## Migration Path to Python/MLX (Future)

**If CPU performance isn't sufficient:**

1. **Build Python training service:**
   ```
   python/
   ├─ requirements.txt (MLX, sentence-transformers)
   ├─ train_embeddings.py
   └─ export_to_onnx.py
   ```

2. **Fine-tune BGE model on trading patterns:**
   - Collect positive/negative outcome pairs
   - Use contrastive learning
   - Export to ONNX format

3. **Load in FastEmbed-rs:**
   - FastEmbed supports custom ONNX models
   - Maintains Rust architecture

4. **Or run Python microservice:**
   - gRPC endpoint for embedding generation
   - Rust core calls Python for embeddings
   - Keeps live trading in Rust

---

## Summary

This plan consolidates RAG implementation into a **2-3 week MVP** with:

✅ **Pure Rust** architecture (consistent with codebase)
✅ **No GPU required initially** (<500ms latency tolerance)
✅ **Clear upgrade path** to Python/MLX for fine-tuning
✅ **Local embeddings** (FastEmbed) + vector DB (Qdrant)
✅ **Historical pattern context** enriching LLM decisions
✅ **Measurable improvement** over baseline signals

The system provides **empirical evidence** about what typically happens in similar market conditions, allowing the LLM to make data-driven decisions instead of relying on general trading knowledge alone.
