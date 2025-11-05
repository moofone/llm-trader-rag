# llm-trader-rag JSON-RPC API Specification

## Overview

This document defines the JSON-RPC 2.0 API that `workflow-manager` uses to query RAG (Retrieval-Augmented Generation) data from the `llm-trader-rag` service.

## Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                     workflow-manager                           │
│  (receives JSON from llm-trader-data)                          │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  1. Receives market data snapshot from llm-trader-data        │
│  2. Query RAG service via JSON-RPC                            │
│  3. Receive historical pattern matches                        │
│  4. Send to LLM with RAG context                              │
│  5. Execute trading decision                                  │
│                                                                │
└───────────────────────┬────────────────────────────────────────┘
                        │ JSON-RPC 2.0
                        │ (TCP socket)
                        ↓
         ┌──────────────────────────────────┐
         │      llm-trader-rag              │
         │   (RAG Query Service)            │
         ├──────────────────────────────────┤
         │                                  │
         │  - Receives query request        │
         │  - Generates embedding           │
         │  - Searches Qdrant vector DB     │
         │  - Returns historical matches    │
         │                                  │
         └──────────────────────────────────┘
```

## JSON-RPC 2.0 Protocol

### Transport
- **Protocol**: TCP socket
- **Default Port**: 7879 (configurable)
- **Format**: JSON-RPC 2.0 over TCP

### Request Format
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "rag.query_patterns",
  "params": {
    // method-specific parameters
  }
}
```

### Success Response Format
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    // method-specific result
  }
}
```

### Error Response Format
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "Error message",
    "data": {
      // optional error details
    }
  }
}
```

## RPC Method: `rag.query_patterns`

### Description
Query similar historical patterns based on current market state.

### Parameters

```json
{
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
    "open_interest_avg_24h": 1450000000.0,
    "price_change_1h": 1.2,
    "price_change_4h": 2.8
  },
  "query_config": {
    "lookback_days": 90,
    "top_k": 5,
    "min_similarity": 0.7,
    "include_regime_filters": true
  }
}
```

#### Parameter Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `symbol` | string | Yes | Trading pair (e.g., "BTCUSDT") |
| `timestamp` | number | Yes | Current timestamp in milliseconds |
| `current_state` | object | Yes | Current market indicators |
| `current_state.price` | number | Yes | Current price |
| `current_state.rsi_7` | number | Yes | 7-period RSI |
| `current_state.rsi_14` | number | Yes | 14-period RSI |
| `current_state.macd` | number | Yes | MACD line value |
| `current_state.ema_20` | number | Yes | 20-period EMA (3m) |
| `current_state.ema_20_4h` | number | Yes | 20-period EMA (4h) |
| `current_state.ema_50_4h` | number | Yes | 50-period EMA (4h) |
| `current_state.funding_rate` | number | Yes | Current funding rate |
| `current_state.open_interest_latest` | number | Yes | Latest open interest |
| `current_state.open_interest_avg_24h` | number | Yes | 24h average OI |
| `current_state.price_change_1h` | number | No | 1-hour price change % |
| `current_state.price_change_4h` | number | No | 4-hour price change % |
| `query_config` | object | No | Query configuration (uses defaults if omitted) |
| `query_config.lookback_days` | number | No | Days to look back (default: 90) |
| `query_config.top_k` | number | No | Max results (default: 5) |
| `query_config.min_similarity` | number | No | Minimum similarity score (default: 0.7) |
| `query_config.include_regime_filters` | boolean | No | Apply OI/funding filters (default: true) |

### Response

```json
{
  "matches": [
    {
      "similarity": 0.89,
      "timestamp": 1725552000000,
      "date": "2025-09-05T14:00:00Z",
      "market_state": {
        "rsi_7": 82.1,
        "rsi_14": 76.8,
        "macd": 68.4,
        "ema_ratio": 1.009,
        "oi_delta_pct": 4.2,
        "funding_rate": 0.00015
      },
      "outcomes": {
        "outcome_1h": -0.8,
        "outcome_4h": -2.3,
        "outcome_24h": -4.1,
        "max_runup_1h": 0.5,
        "max_drawdown_1h": -2.5,
        "hit_stop_loss": true,
        "hit_take_profit": false
      }
    },
    {
      "similarity": 0.87,
      "timestamp": 1724342400000,
      "date": "2025-08-22T16:00:00Z",
      "market_state": {
        "rsi_7": 84.3,
        "rsi_14": 78.5,
        "macd": 71.2,
        "ema_ratio": 1.011,
        "oi_delta_pct": 5.1,
        "funding_rate": 0.00012
      },
      "outcomes": {
        "outcome_1h": 0.3,
        "outcome_4h": 1.1,
        "outcome_24h": 2.8,
        "max_runup_1h": 1.5,
        "max_drawdown_1h": -0.2,
        "hit_stop_loss": false,
        "hit_take_profit": true
      }
    }
  ],
  "statistics": {
    "total_matches": 5,
    "avg_similarity": 0.85,
    "similarity_range": [0.81, 0.89],
    "outcome_4h": {
      "mean": -0.51,
      "median": -0.3,
      "p10": -2.5,
      "p90": 1.2,
      "positive_count": 2,
      "negative_count": 3,
      "win_rate": 0.4
    },
    "stop_loss_hits": 3,
    "take_profit_hits": 1
  },
  "metadata": {
    "query_duration_ms": 145,
    "embedding_duration_ms": 42,
    "retrieval_duration_ms": 98,
    "filters_applied": ["symbol", "timerange", "oi_delta", "funding_sign"],
    "schema_version": 1,
    "feature_version": "v1_nofx_3m4h",
    "embedding_model": "bge-small-en-v1.5"
  }
}
```

#### Response Schema

| Field | Type | Description |
|-------|------|-------------|
| `matches` | array | Array of historical pattern matches |
| `matches[].similarity` | number | Cosine similarity score (0.0-1.0) |
| `matches[].timestamp` | number | Historical timestamp in ms |
| `matches[].date` | string | ISO 8601 formatted date |
| `matches[].market_state` | object | Market indicators at that time |
| `matches[].market_state.rsi_7` | number | RSI(7) value |
| `matches[].market_state.rsi_14` | number | RSI(14) value |
| `matches[].market_state.macd` | number | MACD value |
| `matches[].market_state.ema_ratio` | number | EMA(20)/EMA(50) ratio |
| `matches[].market_state.oi_delta_pct` | number | OI % change vs 24h avg |
| `matches[].market_state.funding_rate` | number | Funding rate at that time |
| `matches[].outcomes` | object | What happened after this state |
| `matches[].outcomes.outcome_1h` | number | Price % change after 1 hour |
| `matches[].outcomes.outcome_4h` | number | Price % change after 4 hours |
| `matches[].outcomes.outcome_24h` | number | Price % change after 24 hours |
| `matches[].outcomes.max_runup_1h` | number | Max positive % move in 1h |
| `matches[].outcomes.max_drawdown_1h` | number | Max negative % move in 1h |
| `matches[].outcomes.hit_stop_loss` | boolean | Did price hit -2% stop? |
| `matches[].outcomes.hit_take_profit` | boolean | Did price hit +3% target? |
| `statistics` | object | Aggregate statistics across matches |
| `statistics.total_matches` | number | Total patterns found |
| `statistics.avg_similarity` | number | Average similarity score |
| `statistics.similarity_range` | array | [min, max] similarity |
| `statistics.outcome_4h` | object | Stats for 4h outcomes |
| `statistics.outcome_4h.mean` | number | Average 4h outcome |
| `statistics.outcome_4h.median` | number | Median 4h outcome |
| `statistics.outcome_4h.p10` | number | 10th percentile |
| `statistics.outcome_4h.p90` | number | 90th percentile |
| `statistics.outcome_4h.positive_count` | number | # of positive outcomes |
| `statistics.outcome_4h.negative_count` | number | # of negative outcomes |
| `statistics.outcome_4h.win_rate` | number | Positive outcome ratio |
| `statistics.stop_loss_hits` | number | # that hit stop loss |
| `statistics.take_profit_hits` | number | # that hit take profit |
| `metadata` | object | Query metadata |
| `metadata.query_duration_ms` | number | Total query time |
| `metadata.embedding_duration_ms` | number | Time to generate embedding |
| `metadata.retrieval_duration_ms` | number | Time to search Qdrant |
| `metadata.filters_applied` | array | List of filters used |
| `metadata.schema_version` | number | Data schema version |
| `metadata.feature_version` | string | Feature set version |
| `metadata.embedding_model` | string | Model used for embeddings |

### Error Codes

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Missing required fields |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Parameter validation failed |
| -32000 | Server error | Internal server error |
| -32001 | Insufficient matches | Not enough similar patterns found |
| -32002 | Symbol not found | Unknown trading symbol |
| -32003 | Qdrant error | Vector database error |
| -32004 | Embedding error | Failed to generate embedding |

### Example Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "Insufficient matches found",
    "data": {
      "matches_found": 2,
      "min_required": 3,
      "suggestion": "Try increasing lookback_days or reducing min_similarity"
    }
  }
}
```

## Integration with workflow-manager

### Workflow Node Definition

The RAG query should be added as a workflow node in `workflow-manager`:

**File**: `workflow-manager/workflows/llm-trader/nodes/rag-query.yml`

```yaml
name: rag-query
type: tool
description: Query RAG service for similar historical patterns
inputs:
  - name: market_data
    type: object
    required: true
    description: Current market snapshot from llm-trader-data
outputs:
  - name: rag_data
    type: object
    description: Historical pattern matches and statistics
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

### Workflow Integration

**File**: `workflow-manager/workflows/llm-trader/main.yml`

```yaml
name: llm-trader-workflow
version: "1.0"
nodes:
  - id: receive-snapshot
    type: input
    description: Receive market data from llm-trader-data

  - id: query-rag
    type: tool
    ref: nodes/rag-query.yml
    inputs:
      market_data: ${receive-snapshot.output}

  - id: format-llm-prompt
    type: script
    inputs:
      market_data: ${receive-snapshot.output}
      rag_data: ${query-rag.output.rag_data}
    script: |
      // Combine market data + RAG context into LLM prompt
      const prompt = formatPromptWithRAG(inputs.market_data, inputs.rag_data);
      return { prompt };

  - id: call-llm
    type: llm
    inputs:
      prompt: ${format-llm-prompt.output.prompt}
    config:
      model: claude-3-5-sonnet-20241022
      temperature: 0.1

  - id: execute-decision
    type: script
    inputs:
      llm_response: ${call-llm.output}
      market_data: ${receive-snapshot.output}
    script: |
      // Parse LLM response and execute trade
      return executeTradeDecision(inputs.llm_response, inputs.market_data);

edges:
  - from: receive-snapshot
    to: query-rag
  - from: query-rag
    to: format-llm-prompt
  - from: format-llm-prompt
    to: call-llm
  - from: call-llm
    to: execute-decision
```

## JSON Schema Definitions

### Request Schema

**File**: `workflow-manager/schemas/rag-query-request.json`

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["symbol", "timestamp", "current_state"],
  "properties": {
    "symbol": {
      "type": "string",
      "pattern": "^[A-Z]+USDT$",
      "description": "Trading pair symbol"
    },
    "timestamp": {
      "type": "number",
      "minimum": 0,
      "description": "Current timestamp in milliseconds"
    },
    "current_state": {
      "type": "object",
      "required": [
        "price", "rsi_7", "rsi_14", "macd", "ema_20",
        "ema_20_4h", "ema_50_4h", "funding_rate",
        "open_interest_latest", "open_interest_avg_24h"
      ],
      "properties": {
        "price": { "type": "number", "minimum": 0 },
        "rsi_7": { "type": "number", "minimum": 0, "maximum": 100 },
        "rsi_14": { "type": "number", "minimum": 0, "maximum": 100 },
        "macd": { "type": "number" },
        "ema_20": { "type": "number", "minimum": 0 },
        "ema_20_4h": { "type": "number", "minimum": 0 },
        "ema_50_4h": { "type": "number", "minimum": 0 },
        "funding_rate": { "type": "number" },
        "open_interest_latest": { "type": "number", "minimum": 0 },
        "open_interest_avg_24h": { "type": "number", "minimum": 0 },
        "price_change_1h": { "type": "number" },
        "price_change_4h": { "type": "number" }
      }
    },
    "query_config": {
      "type": "object",
      "properties": {
        "lookback_days": {
          "type": "number",
          "minimum": 1,
          "maximum": 365,
          "default": 90
        },
        "top_k": {
          "type": "number",
          "minimum": 1,
          "maximum": 50,
          "default": 5
        },
        "min_similarity": {
          "type": "number",
          "minimum": 0.0,
          "maximum": 1.0,
          "default": 0.7
        },
        "include_regime_filters": {
          "type": "boolean",
          "default": true
        }
      }
    }
  }
}
```

### Response Schema

**File**: `workflow-manager/schemas/rag-query-response.json`

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["matches", "statistics", "metadata"],
  "properties": {
    "matches": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["similarity", "timestamp", "date", "market_state", "outcomes"],
        "properties": {
          "similarity": {
            "type": "number",
            "minimum": 0.0,
            "maximum": 1.0
          },
          "timestamp": { "type": "number" },
          "date": { "type": "string", "format": "date-time" },
          "market_state": {
            "type": "object",
            "properties": {
              "rsi_7": { "type": "number" },
              "rsi_14": { "type": "number" },
              "macd": { "type": "number" },
              "ema_ratio": { "type": "number" },
              "oi_delta_pct": { "type": "number" },
              "funding_rate": { "type": "number" }
            }
          },
          "outcomes": {
            "type": "object",
            "properties": {
              "outcome_1h": { "type": ["number", "null"] },
              "outcome_4h": { "type": ["number", "null"] },
              "outcome_24h": { "type": ["number", "null"] },
              "max_runup_1h": { "type": ["number", "null"] },
              "max_drawdown_1h": { "type": ["number", "null"] },
              "hit_stop_loss": { "type": ["boolean", "null"] },
              "hit_take_profit": { "type": ["boolean", "null"] }
            }
          }
        }
      }
    },
    "statistics": {
      "type": "object",
      "required": ["total_matches", "outcome_4h"],
      "properties": {
        "total_matches": { "type": "number" },
        "avg_similarity": { "type": "number" },
        "similarity_range": {
          "type": "array",
          "items": { "type": "number" },
          "minItems": 2,
          "maxItems": 2
        },
        "outcome_4h": {
          "type": "object",
          "properties": {
            "mean": { "type": "number" },
            "median": { "type": "number" },
            "p10": { "type": "number" },
            "p90": { "type": "number" },
            "positive_count": { "type": "number" },
            "negative_count": { "type": "number" },
            "win_rate": { "type": "number", "minimum": 0, "maximum": 1 }
          }
        }
      }
    },
    "metadata": {
      "type": "object",
      "required": ["query_duration_ms", "schema_version"],
      "properties": {
        "query_duration_ms": { "type": "number" },
        "embedding_duration_ms": { "type": "number" },
        "retrieval_duration_ms": { "type": "number" },
        "filters_applied": {
          "type": "array",
          "items": { "type": "string" }
        },
        "schema_version": { "type": "number" },
        "feature_version": { "type": "string" },
        "embedding_model": { "type": "string" }
      }
    }
  }
}
```

## Testing

### Test with curl

```bash
# Start llm-trader-rag JSON-RPC server
cargo run --bin rag-rpc-server -- --port 7879

# Test query
echo '{
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
      "top_k": 5
    }
  }
}' | nc localhost 7879
```

### Test from workflow-manager

```javascript
// In workflow-manager node
const { sendRPCRequest } = require('./rpc-client');

const request = {
  method: 'rag.query_patterns',
  params: {
    symbol: 'BTCUSDT',
    timestamp: Date.now(),
    current_state: marketData,
    query_config: {
      lookback_days: 90,
      top_k: 5
    }
  }
};

const result = await sendRPCRequest('localhost', 7879, request);
console.log('RAG matches:', result.matches);
```

## Configuration

### llm-trader-rag Server Config

**File**: `config/rpc_server.toml`

```toml
[server]
host = "0.0.0.0"
port = 7879
max_connections = 100
timeout_ms = 10000

[rag]
qdrant_url = "http://localhost:6333"
collection_name = "trading_patterns"
embedding_model = "bge-small-en-v1.5"
cache_embeddings = true
cache_ttl_seconds = 300

[query_defaults]
lookback_days = 90
top_k = 5
min_similarity = 0.7
min_matches = 3

[logging]
level = "info"
format = "json"
```

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Query latency (p50) | < 150ms | Including embedding + search |
| Query latency (p99) | < 500ms | |
| Embedding generation | < 50ms | CPU-based with FastEmbed |
| Qdrant search | < 100ms | With proper indexing |
| Throughput | > 100 req/s | Per server instance |
| Connection pool | 100 | Concurrent connections |

## Security Considerations

1. **Network**: Bind to localhost only in dev; use firewall in prod
2. **Authentication**: Add API key validation if exposed externally
3. **Rate Limiting**: Implement per-client rate limiting
4. **Validation**: Strict JSON schema validation on all inputs
5. **Timeouts**: Enforce query timeouts to prevent resource exhaustion
