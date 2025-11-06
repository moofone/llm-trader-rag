# RAG JSON-RPC API - Quick Start Guide

**⚡ TL;DR:** TCP server on port 7879, send JSON-RPC, get historical pattern matches.

---

## Start Server

```bash
# Quick start (defaults to localhost:7879)
cargo run --release --bin rag-rpc-server

# With options
cargo run --release --bin rag-rpc-server -- \
  --host 0.0.0.0 \
  --port 7879 \
  --qdrant-url http://localhost:6333 \
  --collection-name trading_patterns
```

---

## Test Server

```bash
# Use provided test script
./rag-rpc-server/test_request.sh

# Or manually with netcat
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
    }
  }
}' | nc localhost 7879
```

---

## API Method: `rag.query_patterns`

### Minimal Request

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
    }
  }
}
```

### With Query Config

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "rag.query_patterns",
  "params": {
    "symbol": "BTCUSDT",
    "timestamp": 1730811225000,
    "current_state": { /* ... same as above ... */ },
    "query_config": {
      "lookback_days": 90,
      "top_k": 5,
      "min_similarity": 0.7,
      "include_regime_filters": true
    }
  }
}
```

### Response Structure

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

---

## Error Codes

| Code | Meaning | Action |
|------|---------|--------|
| `-32700` | Parse error | Fix JSON syntax |
| `-32600` | Invalid request | Check request format |
| `-32601` | Method not found | Use `rag.query_patterns` |
| `-32602` | Invalid params | Check required fields |
| `-32603` | Internal error | Check server logs |
| `-32001` | Insufficient matches | Reduce `min_similarity` or increase `lookback_days` |
| `-32002` | Symbol not found | Check symbol format (e.g., "BTCUSDT") |
| `-32003` | Qdrant error | Check Qdrant connectivity |
| `-32004` | Embedding error | Check FastEmbed model |

---

## Required Fields

### current_state (all required)

```typescript
{
  price: number;              // Current price
  rsi_7: number;              // 7-period RSI
  rsi_14: number;             // 14-period RSI
  macd: number;               // MACD line value
  ema_20: number;             // 20-period EMA (3m timeframe)
  ema_20_4h: number;          // 20-period EMA (4h timeframe)
  ema_50_4h: number;          // 50-period EMA (4h timeframe)
  funding_rate: number;       // Current funding rate
  open_interest_latest: number;    // Latest OI
  open_interest_avg_24h: number;   // 24h average OI

  // Optional
  price_change_1h?: number;   // 1h price change %
  price_change_4h?: number;   // 4h price change %
}
```

### query_config (optional, defaults shown)

```typescript
{
  lookback_days?: number;     // Default: 90
  top_k?: number;             // Default: 5
  min_similarity?: number;    // Default: 0.7
  include_regime_filters?: boolean;  // Default: true
}
```

---

## Integration Example (Node.js)

```javascript
const net = require('net');

function queryRAG(marketData) {
  return new Promise((resolve, reject) => {
    const client = net.connect(7879, 'localhost', () => {
      const request = {
        jsonrpc: '2.0',
        id: Date.now(),
        method: 'rag.query_patterns',
        params: {
          symbol: 'BTCUSDT',
          timestamp: Date.now(),
          current_state: marketData
        }
      };

      client.write(JSON.stringify(request) + '\n');
    });

    let buffer = '';
    client.on('data', (data) => {
      buffer += data.toString();
      try {
        const response = JSON.parse(buffer);
        client.end();

        if (response.error) {
          reject(new Error(response.error.message));
        } else {
          resolve(response.result);
        }
      } catch (e) {
        // Incomplete JSON, wait for more data
      }
    });

    client.on('error', reject);
    client.setTimeout(5000, () => {
      client.destroy();
      reject(new Error('Request timeout'));
    });
  });
}

// Usage
const ragData = await queryRAG({
  price: 68500.50,
  rsi_7: 83.6,
  rsi_14: 78.2,
  macd: 72.8,
  ema_20: 68200.0,
  ema_20_4h: 67800.0,
  ema_50_4h: 67200.0,
  funding_rate: 0.0001,
  open_interest_latest: 1500000000.0,
  open_interest_avg_24h: 1450000000.0
});

console.log('Found', ragData.matches.length, 'similar patterns');
console.log('Win rate:', ragData.statistics.outcome_4h.win_rate);
```

---

## Performance

- **Latency:** ~100-150ms (p50), ~250-500ms (p99)
- **Throughput:** 100+ req/s per server instance
- **Embedding:** ~30-50ms (CPU-based)
- **Vector search:** ~60-100ms (Qdrant)

---

## Troubleshooting

**Q: Connection refused?**
- A: Ensure `rag-rpc-server` is running: `cargo run --bin rag-rpc-server`

**Q: Error -32001 (insufficient matches)?**
- A: Reduce `min_similarity` from 0.7 to 0.6 or increase `lookback_days`

**Q: Error -32003 (Qdrant error)?**
- A: Check Qdrant is running: `docker ps | grep qdrant`
- A: Verify collection exists: `curl http://localhost:6333/collections`

**Q: Slow queries (>500ms)?**
- A: Check Qdrant performance: `curl http://localhost:6333/metrics`
- A: Ensure proper indexing on Qdrant collection

---

## Files

- **Implementation:** `rag-rpc-server/src/handler.rs`
- **Protocol:** `rag-rpc-server/src/protocol.rs`
- **Tests:** `rag-rpc-server/tests/integration_test.rs`
- **Test Script:** `rag-rpc-server/test_request.sh`
- **Full Docs:** `docs/architecture/jsonrpc_api.md`
- **Server README:** `rag-rpc-server/README.md`

---

**Status:** ✅ Production Ready | **Port:** 7879 | **Protocol:** JSON-RPC 2.0 over TCP
