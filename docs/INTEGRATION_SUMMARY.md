# llm-trader-rag Integration Summary

## Overview

This document summarizes how `llm-trader-rag` integrates with the trading ecosystem, clarifying the data flow, service responsibilities, and API contracts.

## Key Architectural Decisions

### 1. No Direct LLM Usage
✅ **Confirmed**: This project does NOT use OpenAI/Anthropic clients directly.
- LLM calls happen in `workflow-manager`, not here
- This is a pure retrieval service

### 2. No Direct Exchange Queries
✅ **Confirmed**: This project does NOT query exchanges (Bybit, Binance).
- `llm-trader-data` is the single source of truth for historical data
- This project reads from LMDB (shared storage)

### 3. JSON-RPC Integration
✅ **Confirmed**: `workflow-manager` queries this service via JSON-RPC.
- Not queried by `llm-trader-data`
- Port 7879 (configurable)
- Method: `rag.query_patterns`

## Data Flow

```
┌─────────────────┐
│ llm-trader-data │
│                 │
│ Fetches from:   │
│ - Bybit         │
│ - Binance       │
│                 │
│ Stores in:      │
│ - LMDB          │
└────────┬────────┘
         │
         │ Sends market snapshot JSON
         │
         ▼
┌─────────────────────────────────────┐
│ workflow-manager                    │
│                                     │
│ 1. Receives market snapshot         │
│ 2. Queries llm-trader-rag (JSON-RPC)│───────┐
│ 3. Formats prompt with RAG context  │       │
│ 4. Calls LLM (OpenAI/Anthropic)     │       │
│ 5. Executes trade decision          │       │
└─────────────────────────────────────┘       │
                                               │
                                               │ JSON-RPC
                                               │ rag.query_patterns
                                               │
                                               ▼
                                    ┌──────────────────────┐
                                    │ llm-trader-rag       │
                                    │ (this project)       │
                                    │                      │
                                    │ 1. Receives query    │
                                    │ 2. Generates embedding│
                                    │ 3. Searches Qdrant   │
                                    │ 4. Returns matches   │
                                    └──────────────────────┘
```

## Service Responsibilities

### llm-trader-rag (This Project)
| Does | Does NOT |
|------|----------|
| ✅ Provides JSON-RPC API (port 7879) | ❌ Query exchanges (Bybit, Binance) |
| ✅ Generates embeddings (FastEmbed) | ❌ Call LLM APIs (OpenAI, Anthropic) |
| ✅ Queries Qdrant vector DB | ❌ Make trading decisions |
| ✅ Returns historical matches | ❌ Fetch historical data |
| ✅ Calculates statistics | ❌ Store market data |
| ✅ Reads from LMDB (read-only) | ❌ Write to LMDB |

### llm-trader-data
- Fetches historical data from exchanges
- Stores data in LMDB
- Sends market snapshots to workflow-manager
- **Does NOT** query llm-trader-rag

### workflow-manager
- Receives market snapshots from llm-trader-data
- Queries llm-trader-rag via JSON-RPC
- Formats LLM prompts with RAG context
- Calls LLM APIs (OpenAI, Anthropic)
- Executes trading workflows

## JSON-RPC API

### Endpoint
```
Protocol: JSON-RPC 2.0 over TCP
Host:     localhost (configurable)
Port:     7879 (configurable)
Method:   rag.query_patterns
```

### Request Example
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "rag.query_patterns",
  "params": {
    "symbol": "BTCUSDT",
    "timestamp": 1730811225000,
    "current_state": {
      "price": 68500.50,
      "rsi_7": 83.6,
      "rsi_14": 78.2,
      "macd": 72.8,
      "ema_20": 68200.0,
      "ema_20_4h": 67800.0,
      "ema_50_4h": 67200.0,
      "funding_rate": 0.0001,
      "open_interest_latest": 1500000000.0,
      "open_interest_avg_24h": 1450000000.0
    },
    "query_config": {
      "lookback_days": 90,
      "top_k": 5,
      "min_similarity": 0.7
    }
  }
}
```

### Response Example
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "matches": [
      {
        "similarity": 0.89,
        "timestamp": 1725552000000,
        "date": "2025-09-05T14:00:00Z",
        "market_state": {
          "rsi_7": 82.1,
          "macd": 68.4,
          "ema_ratio": 1.009,
          "oi_delta_pct": 4.2,
          "funding_rate": 0.00015
        },
        "outcomes": {
          "outcome_1h": -0.8,
          "outcome_4h": -2.3,
          "outcome_24h": -4.1,
          "hit_stop_loss": true,
          "hit_take_profit": false
        }
      }
    ],
    "statistics": {
      "total_matches": 5,
      "avg_similarity": 0.85,
      "outcome_4h": {
        "mean": -0.51,
        "median": -0.3,
        "win_rate": 0.4
      }
    },
    "metadata": {
      "query_duration_ms": 145,
      "schema_version": 1,
      "feature_version": "v1_nofx_3m4h"
    }
  }
}
```

## Historical Data Integration

### Current Approach: Shared LMDB

```
llm-trader-data (writes) → LMDB ← llm-trader-rag (reads)
```

**Pros**:
- Fast local reads (<10ms)
- No network overhead
- Already using LMDB

**Cons**:
- Must run on same host

**Configuration**:
```toml
[historical_data]
source = "lmdb"
lmdb_path = "/shared/data/trading/lmdb"
read_only = true
```

### Future Options

1. **File Export (S3/Parquet)** - For distributed deployment
2. **API Endpoints** - For service-to-service isolation
3. **Shared Database** - For SQL queries and time-series

See `docs/architecture/architecture.md` Phase 7 for details.

## workflow-manager Integration

### Workflow Node Definition

**File**: `workflow-manager/workflows/llm-trader/nodes/rag-query.yml`

```yaml
name: rag-query
type: tool
description: Query RAG service for similar historical patterns
inputs:
  - name: market_data
    type: object
    required: true
outputs:
  - name: rag_data
    type: object
config:
  rpc:
    host: localhost
    port: 7879
    method: rag.query_patterns
    timeout_ms: 5000
  query:
    lookback_days: 90
    top_k: 5
    min_similarity: 0.7
```

### Workflow Graph

```yaml
nodes:
  - id: receive-snapshot
    type: input

  - id: query-rag
    type: tool
    ref: nodes/rag-query.yml

  - id: format-llm-prompt
    type: script
    # Combines market_data + rag_data

  - id: call-llm
    type: llm
    config:
      model: claude-3-5-sonnet-20241022

  - id: execute-decision
    type: script

edges:
  - receive-snapshot → query-rag
  - query-rag → format-llm-prompt
  - format-llm-prompt → call-llm
  - call-llm → execute-decision
```

## Implementation Checklist

### Phase 1: Historical Data Ingestion ✅
- [x] MarketStateSnapshot structure
- [x] Snapshot formatter (natural language)
- [x] Snapshot extractor (LMDB)
- [x] Vector store (Qdrant)
- [x] Ingestion pipeline (FastEmbed)
- [x] CLI tool (rag-ingest)

### Phase 2: Pattern Retrieval ✅
- [x] RagRetriever (similarity search)
- [x] HistoricalMatch structure
- [x] Prompt formatter (with RAG context)

### Phase 3: LLM Client ❌ (Not in this project)
- This happens in `workflow-manager`, not here

### Phase 4: JSON-RPC Server 📝 (To Implement)
- [ ] JSON-RPC server binary (`rag-rpc-server`)
- [ ] Request/response types
- [ ] Connection handling
- [ ] Error handling
- [ ] Metrics/logging

### Phase 5: workflow-manager Integration 📝 (To Implement)
- [ ] Workflow node definition (YAML)
- [ ] JSON schemas (request/response)
- [ ] Prompt formatter script (TypeScript)
- [ ] End-to-end testing

### Phase 6: Historical Data Integration 📝 (To Implement)
- [ ] Shared LMDB access
- [ ] Data schema validation
- [ ] Freshness monitoring
- [ ] Schema version enforcement

## Testing Strategy

### Unit Tests ✅
- All core components have unit tests
- Mock data for testing without Qdrant

### Integration Tests (Requires Qdrant) ⏳
```bash
# Start Qdrant
docker run -d -p 6333:6333 qdrant/qdrant

# Run ingestion
cargo run --bin rag-ingest -- --symbols BTCUSDT --start 7 --end now

# Test query
cargo test --package trading-data-services test_ingestion_pipeline -- --ignored
```

### JSON-RPC Tests (To Implement) 📝
```bash
# Start RPC server
cargo run --bin rag-rpc-server -- --port 7879

# Test with curl
echo '{"jsonrpc":"2.0","id":1,"method":"rag.query_patterns","params":{...}}' | nc localhost 7879
```

### Workflow Tests (To Implement) 📝
```bash
# In workflow-manager
npm test -- --grep "RAG integration"
```

## Documentation

### Created Documents
1. ✅ `docs/architecture/architecture.md` - Service architecture and responsibilities
2. ✅ `docs/architecture/jsonrpc_api.md` - Complete JSON-RPC API specification
3. ✅ `spec/LLM_BOT_RAG_IMPLEMENTATION.md` - Phase 4 (JSON-RPC integration)
4. ✅ `spec/LLM_BOT_RAG_IMPLEMENTATION.md` - Phase 7 (Historical data integration)
5. ✅ `docs/INTEGRATION_SUMMARY.md` - This document

### Additional Resources
- `README.md` - Project overview and usage
- `PHASE1_STATUS.md` - Phase 1 & 2 completion status
- `docs/PHASE_4_IMPLEMENTATION.md` - Phase 4 strategy plugin (deprecated)

## Next Steps

### For Implementation (Future)
1. Implement JSON-RPC server (`rag-rpc-server/src/main.rs`)
2. Create workflow node in `workflow-manager`
3. Add JSON schemas to `workflow-manager/schemas/`
4. Implement prompt formatting script
5. End-to-end testing with mock workflow

### For Testing
1. Set up shared LMDB with `llm-trader-data`
2. Test data consistency and freshness
3. Load test JSON-RPC server
4. Validate latency targets (<150ms p50)

### For Deployment
1. Configure ports and hosts
2. Set up monitoring and alerting
3. Add authentication (if exposed externally)
4. Implement rate limiting

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Query latency (p50) | < 150ms | To measure |
| Query latency (p99) | < 500ms | To measure |
| Embedding generation | < 50ms | Expected (CPU) |
| Qdrant search | < 100ms | Expected |
| Throughput | > 100 req/s | To measure |

## Configuration

### llm-trader-rag
```toml
[server]
host = "0.0.0.0"
port = 7879

[rag]
qdrant_url = "http://localhost:6333"
collection_name = "trading_patterns"
min_matches = 3

[historical_data]
source = "lmdb"
lmdb_path = "/shared/data/trading/lmdb"
read_only = true
```

### workflow-manager (workflow config)
```yaml
config:
  rpc:
    host: localhost
    port: 7879
    timeout_ms: 5000
  query:
    lookback_days: 90
    top_k: 5
    min_similarity: 0.7
```

## Questions & Answers

**Q: Does llm-trader-rag call LLMs?**
A: No. LLM calls happen in `workflow-manager`.

**Q: Does llm-trader-rag query exchanges?**
A: No. `llm-trader-data` fetches from exchanges and stores in LMDB.

**Q: Who queries llm-trader-rag?**
A: `workflow-manager` queries it via JSON-RPC.

**Q: How does historical data flow?**
A: `llm-trader-data` → LMDB → `llm-trader-rag` (read-only)

**Q: What port does the RPC server use?**
A: 7879 (configurable)

**Q: What's the RPC method name?**
A: `rag.query_patterns`

## References

- **Main Spec**: `spec/LLM_BOT_RAG_IMPLEMENTATION.md`
- **API Spec**: `docs/architecture/jsonrpc_api.md`
- **Architecture**: `docs/architecture/architecture.md`
- **Status**: `PHASE1_STATUS.md`
- **workflow-manager**: `../workflow-manager/` (RPC client examples)
