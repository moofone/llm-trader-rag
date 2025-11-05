# llm-trader-rag Architecture

## Service Layer Design

**Important**: This project (`llm-trader-rag`) does NOT use OpenAI or any LLM clients directly. It is designed as a service layer that is queried by `../llm-trader-data`.

## Data Flow

```
llm-trader-data → sends market snapshot JSON → workflow-manager
                                                      ↓
                                         workflow-manager queries via JSON-RPC
                                                      ↓
                                               llm-trader-rag
                                               (this service)
                                                      ↓
                                         returns historical matches + statistics
                                                      ↓
                                         workflow-manager formats prompt with RAG
                                                      ↓
                                         workflow-manager → LLM → trade decision
```

## Service Responsibilities

### llm-trader-rag (This Project)
- **Does NOT query exchanges** - no direct API calls to Bybit/Binance
- **Does NOT call LLMs** - no OpenAI/Anthropic clients
- **Does**:
  - Provides JSON-RPC server (port 7879)
  - Queries Qdrant vector database
  - Returns historical pattern matches
  - Pure retrieval service

### llm-trader-data
- Fetches historical data from exchanges (Bybit, Binance)
- Stores data in LMDB
- Sends market snapshots to workflow-manager

### workflow-manager
- Receives market snapshots from llm-trader-data
- Queries llm-trader-rag for historical patterns (JSON-RPC)
- Formats LLM prompts with RAG context
- Calls LLM APIs (OpenAI, Anthropic)
- Executes trading decisions

## Architecture Components

### llm-trader-rag (This Project)
- **Purpose**: RAG (Retrieval-Augmented Generation) retrieval service
- **Responsibilities**:
  - Exposes JSON-RPC 2.0 API (port 7879)
  - Generates embeddings from current market state
  - Queries Qdrant vector database for similar patterns
  - Returns historical matches with outcomes
  - Calculates aggregate statistics
- **Does NOT**:
  - Make direct LLM API calls (OpenAI, Anthropic, etc.)
  - Query exchanges (Bybit, Binance, etc.)
  - Make trading decisions

## JSON-RPC API

### Endpoint
- **Host**: localhost (configurable)
- **Port**: 7879 (configurable)
- **Protocol**: JSON-RPC 2.0 over TCP

### Method: `rag.query_patterns`

**Request**:
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

**Response**:
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

See `docs/architecture/jsonrpc_api.md` for complete API specification

### llm-trader-data
- Single source of truth for historical data
- Fetches from exchanges (Bybit, Binance)
- Stores in LMDB
- Sends market snapshots to workflow-manager

### workflow-manager
- Receives JSON snapshots from llm-trader-data
- Queries llm-trader-rag via JSON-RPC
- Formats prompts with RAG context
- Calls LLM APIs (OpenAI, Anthropic)
- Executes trading workflows and decisions

## Design Rationale

This separation of concerns allows:
1. **Modularity**: RAG functionality is isolated and reusable
2. **Scalability**: Each service can scale independently
3. **Separation of Concerns**: Data retrieval vs. LLM interaction vs. workflow management
4. **Testing**: RAG functionality can be tested without LLM API calls

## No Direct LLM Usage

This project deliberately avoids:
- OpenAI client imports
- Direct API calls to LLM providers
- LLM-specific configuration or credentials
- Token management or rate limiting for LLMs

All LLM interactions happen in the workflow-manager, not in this service.
