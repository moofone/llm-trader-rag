# LLM Trader RAG - Phase 1 Implementation

**Status:** Phase 1 Complete - Vector Database & Historical Data Ingestion
**Language:** Pure Rust
**Architecture:** Qdrant + FastEmbed-rs + Async LLM SDK

## Overview

This project implements a Retrieval-Augmented Generation (RAG) system for an LLM-powered trading bot. The system provides historical pattern context to enhance trade signal generation, allowing the LLM to see "what happened the last 5 times the market looked like this" and make evidence-based decisions.

## Architecture

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
Generate Embeddings (FastEmbed-rs + BGE-small-en-v1.5)
        │
        └─ text → 384-dim vector
        │
        ▼
Store in Qdrant Vector Database
        │
        └─ Vector + metadata (outcomes, indicators, timestamp)
```

## Project Structure

```
llm-trader-rag/
├── trading-core/              # Core types and data structures
│   ├── src/
│   │   ├── types/
│   │   │   └── market_snapshot.rs  # MarketStateSnapshot structure
│   │   └── lib.rs
│   └── Cargo.toml
│
├── trading-data-services/     # RAG data pipeline
│   ├── src/
│   │   ├── rag/
│   │   │   ├── snapshot_formatter.rs    # Natural language conversion
│   │   │   ├── snapshot_extractor.rs    # LMDB extraction
│   │   │   ├── vector_store.rs          # Qdrant integration
│   │   │   └── ingestion_pipeline.rs    # End-to-end pipeline
│   │   └── lib.rs
│   └── Cargo.toml
│
├── trading-strategy/          # LLM strategy integration (Phase 2+)
│   └── Cargo.toml
│
├── rag-ingest/                # CLI tool for data ingestion
│   ├── src/
│   │   └── main.rs           # Command-line interface
│   └── Cargo.toml
│
├── spec/                      # Specifications
│   └── LLM_BOT_RAG_IMPLEMENTATION.md
│
└── Cargo.toml                 # Workspace configuration
```

## Phase 1 Components

### 1. MarketStateSnapshot (`trading-core/src/types/market_snapshot.rs`)

Core data structure capturing market state and outcomes:

- **Identification**: Symbol, timestamp, price
- **Current Indicators** (3m timeframe): RSI(7/14), MACD, EMA(20)
- **Time Series** (last 10 points): Price, EMA, MACD, RSI series
- **Longer-term Context** (4h timeframe): EMA(20/50), ATR, volume, MACD, RSI
- **Market Microstructure**: Open interest, funding rate, price changes
- **Outcomes** (calculated from future data): 15m/1h/4h/24h price changes
- **Outcome Metadata**: Max runup/drawdown, stop/target hits

### 2. Snapshot Formatter (`trading-data-services/src/rag/snapshot_formatter.rs`)

Converts snapshots to natural language for embeddings:

- **Detailed format**: Semantic interpretations ("RSI is extremely overbought")
- **Simple format**: Numerical values for faster processing
- Interprets indicators with context (trend, momentum, volatility)

### 3. Snapshot Extractor (`trading-data-services/src/rag/snapshot_extractor.rs`)

Extracts historical snapshots from LMDB:

- Configurable time ranges and intervals
- Mock data implementation (for testing without LMDB)
- TODO: Integration with actual LMDB manager

### 4. Vector Store (`trading-data-services/src/rag/vector_store.rs`)

Qdrant vector database integration:

- Collection management (auto-create with 384 dimensions)
- Batch upsert operations
- Similarity search with filtering
- Rich metadata storage (outcomes, indicators, provenance)

### 5. Ingestion Pipeline (`trading-data-services/src/rag/ingestion_pipeline.rs`)

End-to-end data ingestion:

1. Extract snapshots from LMDB
2. Convert to natural language text
3. Generate embeddings (BGE-small-en-v1.5)
4. Upload to Qdrant with metadata

Features:
- Batch processing (100 snapshots per batch)
- Progress logging
- Statistics reporting
- Multi-symbol support

### 6. CLI Tool (`rag-ingest/src/main.rs`)

Command-line interface for data ingestion:

```bash
# Ingest 90 days of BTC and ETH data at 15-minute intervals
cargo run --bin rag-ingest

# Custom configuration
cargo run --bin rag-ingest -- \
  --symbols BTCUSDT,ETHUSDT,SOLUSDT \
  --start 2025-10-01T00:00:00Z \
  --end now \
  --interval 15 \
  --qdrant-url http://localhost:6333 \
  --collection trading_patterns \
  --log-level info
```

## Build Instructions

### Known Issue: ONNX Runtime TLS Certificate

The current build has a TLS certificate issue when downloading the ONNX Runtime in Docker environments. This is due to certificate validation failures.

**Workarounds:**

1. **Development (Local Machine):**
   ```bash
   cargo build --release
   ```
   Should work on local machines with proper certificates.

2. **Docker/CI Environments:**
   The build currently fails due to TLS issues. Solutions:

   a) Pre-download ONNX Runtime and set `ORT_LIB_LOCATION`:
   ```bash
   export ORT_LIB_LOCATION=/path/to/onnxruntime
   cargo build --release
   ```

   b) Use system ONNX Runtime (if available):
   ```bash
   export ORT_STRATEGY=system
   cargo build --release
   ```

   c) Configure TLS certificates (production):
   ```bash
   # Update CA certificates in container
   apt-get update && apt-get install -y ca-certificates
   update-ca-certificates
   ```

### Build Commands

```bash
# Build all crates
cargo build

# Build release version
cargo build --release

# Build specific binary
cargo build --bin rag-ingest

# Run tests
cargo test

# Run CLI tool
cargo run --bin rag-ingest -- --help
```

## Testing

### Unit Tests (No Qdrant Required)

Run all unit tests (mock data only):

```bash
# Run all tests
export ORT_LIB_LOCATION=/tmp/onnxruntime/onnxruntime  # If needed for ONNX Runtime
cargo test

# Run tests for specific package
cargo test --package trading-core
cargo test --package trading-data-services
cargo test --package trading-strategy

# Run Phase 2 integration tests (prompt generation, no Qdrant needed)
cargo test --package trading-strategy --test phase2_integration_test -- --nocapture

# Run with output
cargo test -- --nocapture

# Test results summary
# - trading-core: 4 tests
# - trading-data-services: 5 tests (1 ignored - requires Qdrant)
# - rag-ingest: 2 tests
# - trading-strategy: 4 unit tests + 3 integration tests
```

### Integration Tests (Requires Qdrant)

**Step 1: Start Qdrant**

```bash
# Using Docker (recommended)
docker run -d -p 6333:6333 -p 6334:6334 --name qdrant qdrant/qdrant

# Verify it's running
curl http://localhost:6333/
```

**Step 2: Run Integration Tests**

```bash
# Run the ignored integration test for ingestion pipeline
export ORT_LIB_LOCATION=/tmp/onnxruntime/onnxruntime  # If needed
cargo test --package trading-data-services test_ingestion_pipeline -- --ignored --nocapture

# This test will:
# 1. Create a test collection in Qdrant
# 2. Generate 100 mock market snapshots
# 3. Convert to natural language
# 4. Generate embeddings (384-dim vectors)
# 5. Upload to Qdrant
# 6. Verify successful storage
```

**Step 3: Manual End-to-End Test**

```bash
# 1. Start Qdrant (if not already running)
docker start qdrant

# 2. Run the ingestion CLI (uses mock data)
export ORT_LIB_LOCATION=/tmp/onnxruntime/onnxruntime
cargo run --bin rag-ingest -- \
  --symbols BTCUSDT \
  --start 7 \
  --end now \
  --interval 60 \
  --log-level debug

# 3. Verify data in Qdrant
curl http://localhost:6333/collections/trading_patterns

# Expected output:
# {
#   "result": {
#     "status": "green",
#     "vectors_count": <number>,
#     "points_count": <number>,
#     ...
#   }
# }

# 4. Query a sample point
curl -X POST http://localhost:6333/collections/trading_patterns/points/scroll \
  -H 'Content-Type: application/json' \
  -d '{"limit": 1, "with_payload": true, "with_vector": false}'
```

**Step 4: Phase 2 RAG Retrieval Test**

Once you have data in Qdrant, you can test the pattern retrieval:

```bash
# Run Phase 2 integration tests
cargo test --package trading-strategy --test phase2_integration_test -- --nocapture

# This test verifies:
# 1. Baseline prompt generation (no RAG)
# 2. RAG-enhanced prompt with historical matches
# 3. Statistical analysis (percentiles, win rates)
# 4. Edge cases (oversold, minimal matches, empty results)
```

### Test Coverage

Current test coverage:

- **trading-core**: Market snapshot creation, field calculations
- **trading-data-services**: Snapshot formatting, extraction, vector store operations, ingestion pipeline
- **rag-ingest**: CLI argument parsing, date parsing
- **trading-strategy (Phase 2)**:
  - Historical match creation
  - Baseline prompt formatting
  - RAG-enhanced prompt formatting
  - Statistical outcome analysis
  - End-to-end prompt generation flow

### Troubleshooting Tests

**ONNX Runtime Issues:**

```bash
# If you see TLS certificate errors during build/test:
# 1. Manually download ONNX Runtime
curl -k -L https://github.com/microsoft/onnxruntime/releases/download/v1.20.0/onnxruntime-linux-x64-1.20.0.tgz -o /tmp/onnxruntime.tgz
mkdir -p /tmp/onnxruntime
tar -xzf /tmp/onnxruntime.tgz -C /tmp/onnxruntime --strip-components=1

# 2. Set environment variable
export ORT_LIB_LOCATION=/tmp/onnxruntime/lib

# 3. Run tests
cargo test
```

**Qdrant Connection Issues:**

```bash
# Check if Qdrant is running
curl http://localhost:6333/

# Check Docker container status
docker ps | grep qdrant

# View Qdrant logs
docker logs qdrant

# Restart Qdrant
docker restart qdrant
```

## Usage

### Prerequisites

#### 1. Qdrant Vector Database

**Option A: Docker (Recommended)**

Start Qdrant in a Docker container:

```bash
# Run Qdrant with default settings
docker run -p 6333:6333 -p 6334:6334 \
  -v $(pwd)/qdrant_storage:/qdrant/storage:z \
  qdrant/qdrant

# Run in background (detached mode)
docker run -d -p 6333:6333 -p 6334:6334 \
  --name qdrant \
  -v $(pwd)/qdrant_storage:/qdrant/storage:z \
  qdrant/qdrant

# Check if Qdrant is running
curl http://localhost:6333/collections

# View logs
docker logs qdrant

# Stop Qdrant
docker stop qdrant

# Start existing container
docker start qdrant
```

**Option B: Docker Compose**

Create `docker-compose.yml`:

```yaml
version: '3.8'
services:
  qdrant:
    image: qdrant/qdrant:latest
    ports:
      - "6333:6333"  # REST API
      - "6334:6334"  # gRPC API
    volumes:
      - ./qdrant_storage:/qdrant/storage
    restart: unless-stopped
```

Then run:

```bash
docker-compose up -d
docker-compose logs -f qdrant
docker-compose down
```

**Option C: Native Installation**

Download from https://qdrant.tech/documentation/guides/installation/

```bash
# Linux/macOS
wget https://github.com/qdrant/qdrant/releases/download/v1.7.0/qdrant-x86_64-unknown-linux-gnu.tar.gz
tar -xzf qdrant-x86_64-unknown-linux-gnu.tar.gz
./qdrant

# Or use package manager
# Homebrew (macOS)
brew install qdrant

# Then run
qdrant
```

**Verify Installation:**

```bash
# Check Qdrant is running
curl http://localhost:6333/

# Expected response:
# {"title":"qdrant - vector search engine","version":"1.x.x"}
```

#### 2. LMDB Data (or use mock data for testing)

The current implementation uses mock data for testing. Integration with actual LMDB manager is pending.

### Running Ingestion

```bash
# Basic usage (90 days, BTC & ETH, 15min intervals)
cargo run --bin rag-ingest

# Specify custom date range
cargo run --bin rag-ingest -- --start 30 --end now

# Multiple symbols
cargo run --bin rag-ingest -- --symbols BTCUSDT,ETHUSDT,SOLUSDT,LINKUSDT

# Higher frequency (5-minute snapshots)
cargo run --bin rag-ingest -- --interval 5

# Different Qdrant instance
cargo run --bin rag-ingest -- --qdrant-url https://your-cluster.cloud.qdrant.io

# Debug logging
cargo run --bin rag-ingest -- --log-level debug
```

### Output

```
🚀 RAG Historical Data Ingestion Tool
=====================================
Configuration:
  Symbols: ["BTCUSDT", "ETHUSDT"]
  Start: 2025-08-06T12:33:45Z (1722951225000)
  End: 2025-11-05T12:33:45Z (1730811225000)
  Interval: 15 minutes
  Qdrant URL: http://localhost:6333
  Collection: trading_patterns

Initializing ingestion pipeline...
Loading embedding model (BGE-small-en-v1.5)...
Pipeline initialized successfully

Processing symbol: BTCUSDT
Created 8640 snapshots for BTCUSDT
Generating embeddings for batch of 100 snapshots...
Processed 100 embeddings (total: 100)
...
Uploaded 8640 points to Qdrant

Processing symbol: ETHUSDT
Created 8640 snapshots for ETHUSDT
...

✅ Ingestion Complete!
=====================
  BTCUSDT: 8640 snapshots, 8640 embeddings, 8640 points uploaded
  ETHUSDT: 8640 snapshots, 8640 embeddings, 8640 points uploaded
```

## Data Model

### MarketStateSnapshot Fields

- **Identification**: `symbol`, `timestamp`, `price`
- **3m Indicators**: `rsi_7`, `rsi_14`, `macd`, `ema_20`
- **3m Series**: `mid_prices[10]`, `ema_20_values[10]`, `macd_values[10]`, `rsi_7_values[10]`, `rsi_14_values[10]`
- **4h Context**: `ema_20_4h`, `ema_50_4h`, `atr_3_4h`, `atr_14_4h`, `current_volume_4h`, `avg_volume_4h`
- **4h Series**: `macd_4h_values[10]`, `rsi_14_4h_values[10]`
- **Derivatives**: `open_interest_latest`, `open_interest_avg_24h`, `funding_rate`
- **Price Changes**: `price_change_1h`, `price_change_4h`
- **Outcomes**: `outcome_15m`, `outcome_1h`, `outcome_4h`, `outcome_24h`
- **Outcome Metrics**: `max_runup_1h`, `max_drawdown_1h`, `hit_stop_loss`, `hit_take_profit`

### Qdrant Point Payload

Each vector in Qdrant includes:
- All snapshot fields
- Derived features: `ema_ratio`, `oi_delta_pct`, `volatility_ratio`
- Metadata: `schema_version`, `feature_version`, `embedding_model`, `build_id`, `date`

## Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test --package trading-core
cargo test --package trading-data-services

# Run with logging
RUST_LOG=debug cargo test
```

## Next Steps (Phase 2+)

- [ ] **Phase 2**: Live pattern retrieval during trading
- [ ] **Phase 3**: LLM client integration (OpenAI/Anthropic)
- [ ] **Phase 4**: Strategy plugin integration
- [ ] **Phase 5**: Configuration & monitoring
- [ ] **Phase 6**: Functional testing & walk-forward evaluation

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Embedding generation | <50ms | TBD (requires working build) |
| Vector search | <100ms | TBD |
| Snapshot extraction | <10ms | ✅ (mock data) |
| Batch processing | 100 snapshots/batch | ✅ |

## Configuration

### Environment Variables

- `ORT_STRATEGY`: ONNX Runtime download strategy (`download`, `system`)
- `ORT_LIB_LOCATION`: Path to pre-downloaded ONNX Runtime
- `GIT_SHA`: Build identifier (embedded in metadata)
- `RUST_LOG`: Logging level (`trace`, `debug`, `info`, `warn`, `error`)

### Qdrant Configuration

Default: `http://localhost:6333`
- Local embedded: Qdrant runs in-process
- Docker: `docker run -p 6333:6333 qdrant/qdrant`
- Cloud: `https://your-cluster.cloud.qdrant.io`

## Contributing

See `spec/LLM_BOT_RAG_IMPLEMENTATION.md` for detailed implementation plan.

## License

TBD

## Contact

TBD
