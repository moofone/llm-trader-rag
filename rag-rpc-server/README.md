# RAG JSON-RPC Server

JSON-RPC 2.0 server for querying historical trading pattern matches from the RAG vector database. This service is designed to be called by `workflow-manager` during live trading to provide historical context to LLM-based trading decisions.

## Overview

The RAG RPC server exposes a single JSON-RPC method:
- `rag.query_patterns` - Find similar historical market patterns and their outcomes

## Architecture

```
workflow-manager → JSON-RPC (TCP 7879) → rag-rpc-server
                                              ↓
                                         [FastEmbed]
                                              ↓
                                          [Qdrant]
                                              ↓
                                    Historical Matches + Statistics
```

## Quick Start

### Prerequisites

1. **Qdrant running** on localhost:6333
   ```bash
   docker run -d -p 6333:6333 -p 6334:6334 --name qdrant qdrant/qdrant
   ```

2. **Historical data ingested** into Qdrant collection
   ```bash
   cargo run --bin rag-ingest -- --symbols BTCUSDT,ETHUSDT --start 90 --end now
   ```

### Start Server

```bash
# Default configuration (port 7879, localhost:6333 Qdrant)
cargo run --bin rag-rpc-server

# Custom configuration
cargo run --bin rag-rpc-server -- \
  --host 0.0.0.0 \
  --port 7879 \
  --qdrant-url http://localhost:6333 \
  --collection-name trading_patterns \
  --min-matches 3 \
  --log-level info
```

### Test with netcat

```bash
# Use provided test script
./rag-rpc-server/test_request.sh

# Or manually
echo '{"jsonrpc":"2.0","id":1,"method":"rag.query_patterns","params":{"symbol":"BTCUSDT","timestamp":1730811225000,"current_state":{"price":68500.50,"rsi_7":83.6,"rsi_14":78.2,"macd":72.8,"ema_20":68200.0,"ema_20_4h":67800.0,"ema_50_4h":67200.0,"funding_rate":0.0001,"open_interest_latest":1500000000.0,"open_interest_avg_24h":1450000000.0}}}' | nc localhost 7879
```

## JSON-RPC API

### Method: `rag.query_patterns`

Query for similar historical market patterns.

#### Request

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
}
```

**Parameters:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `symbol` | string | Yes | - | Trading pair (e.g., "BTCUSDT") |
| `timestamp` | number | Yes | - | Current timestamp in milliseconds |
| `current_state` | object | Yes | - | Current market indicators |
| `query_config` | object | No | See below | Query configuration |

**Query Config Defaults:**
- `lookback_days`: 90
- `top_k`: 5
- `min_similarity`: 0.7
- `include_regime_filters`: true

#### Success Response

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
}
```

#### Error Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32001,
    "message": "Insufficient matches: found 2, required 3",
    "data": {
      "matches_found": 2,
      "min_required": 3,
      "suggestion": "Try increasing lookback_days or reducing min_similarity"
    }
  }
}
```

**Error Codes:**

| Code | Message | Description |
|------|---------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Missing required fields |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Parameter validation failed |
| -32603 | Internal error | Server error |
| -32001 | Insufficient matches | Not enough similar patterns found |
| -32002 | Symbol not found | Unknown trading symbol |
| -32003 | Qdrant error | Vector database error |
| -32004 | Embedding error | Failed to generate embedding |

## Configuration

### Command Line Options

```bash
cargo run --bin rag-rpc-server -- --help
```

| Option | Default | Description |
|--------|---------|-------------|
| `--host` | 0.0.0.0 | Server bind address |
| `--port` | 7879 | Server port |
| `--qdrant-url` | http://localhost:6333 | Qdrant URL |
| `--collection-name` | trading_patterns | Qdrant collection name |
| `--min-matches` | 3 | Minimum matches required |
| `--log-level` | info | Log level (trace/debug/info/warn/error) |

### Environment Variables

```bash
export RUST_LOG=rag_rpc_server=debug,trading_strategy=debug
cargo run --bin rag-rpc-server
```

## Testing

### Unit Tests

```bash
cargo test --package rag-rpc-server
```

### Integration Tests

Requires Qdrant running and test data ingested:

```bash
# Start Qdrant
docker run -d -p 6333:6333 qdrant/qdrant

# Ingest test data
cargo run --bin rag-ingest -- --symbols BTCUSDT --start 7 --end now

# Start server (in another terminal)
cargo run --bin rag-rpc-server

# Run integration tests
cargo test --package rag-rpc-server --test integration_test -- --ignored --nocapture

# Or use test script
./rag-rpc-server/test_request.sh
```

## Performance

### Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Query latency (p50) | < 150ms | Including embedding + search |
| Query latency (p99) | < 500ms | |
| Embedding generation | < 50ms | CPU-based with FastEmbed |
| Qdrant search | < 100ms | With proper indexing |
| Throughput | > 100 req/s | Per server instance |

### Monitoring

The server logs include:
- Query duration (total, embedding, retrieval)
- Number of matches found
- Errors and warnings
- Connection events

Example log output:
```
2025-11-05T12:00:00Z INFO rag_rpc_server: RAG query completed: symbol=BTCUSDT, matches=5, duration=145ms
```

## Deployment

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin rag-rpc-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/rag-rpc-server /usr/local/bin/
EXPOSE 7879
CMD ["rag-rpc-server", "--host", "0.0.0.0", "--port", "7879"]
```

### Systemd Service

```ini
[Unit]
Description=RAG JSON-RPC Server
After=network.target

[Service]
Type=simple
User=rag
ExecStart=/usr/local/bin/rag-rpc-server --host 0.0.0.0 --port 7879
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

## Troubleshooting

### Server won't start

**Error:** "Failed to bind to 0.0.0.0:7879"
- **Solution:** Port already in use. Try a different port or stop other service.

**Error:** "Failed to connect to Qdrant"
- **Solution:** Ensure Qdrant is running: `docker ps | grep qdrant`

### No matches found

**Error:** "Insufficient matches: found 0, required 3"
- **Cause:** No data in Qdrant collection
- **Solution:** Run ingestion: `cargo run --bin rag-ingest`

**Error:** "Insufficient matches: found 2, required 3"
- **Cause:** Not enough similar patterns
- **Solution:** Increase `lookback_days` or reduce `min_similarity` in request

### Slow queries

- Check Qdrant indexing status
- Reduce `top_k` parameter
- Monitor embedding generation time

### Connection timeouts

- Increase client timeout (default 5s in workflow-manager)
- Check server load and resources
- Review Qdrant performance

## Integration with workflow-manager

See `docs/architecture/jsonrpc_api.md` for complete integration guide.

### Example workflow node (workflow-manager):

```yaml
name: rag-query
type: tool
description: Query RAG service for similar historical patterns
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

## Development

### Project Structure

```
rag-rpc-server/
├── src/
│   ├── main.rs          # Entry point, CLI
│   ├── server.rs        # TCP server, connection handling
│   ├── handler.rs       # RPC method handler
│   ├── protocol.rs      # JSON-RPC types
│   ├── error.rs         # Error handling
│   └── config.rs        # Configuration
├── tests/
│   └── integration_test.rs
├── test_request.sh      # Manual test script
├── Cargo.toml
└── README.md
```

### Adding New Methods

1. Add method name constant to `protocol.rs`
2. Add request/response types
3. Implement handler in `handler.rs`
4. Route in `server.rs::process_request()`
5. Add tests

## References

- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [API Documentation](../docs/architecture/jsonrpc_api.md)
- [Integration Summary](../docs/INTEGRATION_SUMMARY.md)
- [Main Spec](../spec/LLM_BOT_RAG_IMPLEMENTATION.md)

## License

TBD
