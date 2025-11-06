# Phase 6: LMDB Integration Implementation Plan

**Status:** In Progress
**Goal:** Replace mock data with real historical data from llm-trader-data's LMDB storage

---

## Overview

This phase connects `llm-trader-rag` to the LMDB storage maintained by `llm-trader-data`, establishing it as the single source of truth for historical market data.

### Key Requirements
1. Read-only access to LMDB (no writes)
2. Compatible with llm-trader-data's schema
3. Extract complete snapshots with all required indicators
4. Handle multi-timeframe data (3m + 4h)
5. Graceful error handling for missing data

---

## LMDB Schema (from llm-trader-data)

### Databases
- `candles_3m`: 3-minute OHLCV candles
- `candles_4h`: 4-hour OHLCV candles
- `indicators_3m`: 3-minute technical indicators
- `indicators_4h`: 4-hour technical indicators

### Key Format
```
{symbol}:{timestamp_ms}
Example: BTCUSDT:1730811225000
```

### Value Format (JSON)
```json
// indicators_3m
{
  "ema_20": 68150.0,
  "ema_50": 67900.0,
  "macd": 154.22,
  "rsi_7": 63.5,
  "rsi_14": 58.7,
  "atr_14": 425.5
}

// indicators_4h
{
  "ema_20": 67500.0,
  "ema_50": 66800.0,
  "macd": 98.4,
  "rsi_14": 52.3,
  "atr_3": 385.2,
  "atr_14": 582.1
}
```

### Environment Variables (from llm-trader-data)
- `LMDB_PATH`: `/shared/data/trading/lmdb` (default)
- `LMDB_MAX_SIZE_GB`: 10 (default)
- `LMDB_RETENTION_DAYS`: 400 (default, ~1 year)

---

## Implementation Tasks

### 1. Create LMDB Reader Module ✅ Next
**File:** `trading-data-services/src/rag/lmdb_reader.rs`

**Features:**
- Read-only LMDB environment
- Open named databases (candles_3m, candles_4h, indicators_3m, indicators_4h)
- Key parsing/generation
- JSON deserialization
- Time-range queries
- Symbol filtering

**Dependencies:**
```toml
[dependencies]
lmdb = "0.8"
serde_json = "1.0"
```

### 2. Update Snapshot Extractor
**File:** `trading-data-services/src/rag/snapshot_extractor.rs`

**Changes:**
- Add `LmdbSnapshotExtractor` struct
- Implement data extraction from LMDB
- Build `MarketStateSnapshot` from LMDB data
- Calculate outcomes from future data
- Keep mock extractor for testing

### 3. Data Validation Layer
**Features:**
- Schema version compatibility checks
- Missing data detection
- Indicator range validation (RSI 0-100, etc.)
- Time-series continuity checks
- Null/NaN rejection

### 4. Configuration Updates
**File:** `config/llm_rag_config.toml`

```toml
[ingestion]
data_source = "lmdb"  # or "mock" for testing
lmdb_path = "/shared/data/trading/lmdb"
lmdb_read_only = true
snapshot_interval_minutes = 15
batch_size = 100
symbols = ["BTCUSDT", "ETHUSDT"]

[data_validation]
require_complete_series = true
reject_nan_values = true
min_lookback_points = 10
max_age_seconds = 86400  # 24 hours
```

### 5. CLI Updates
**File:** `rag-ingest/src/main.rs`

**New Arguments:**
```bash
--data-source <lmdb|mock>   # Data source type
--lmdb-path <path>          # Path to LMDB directory
--validate                  # Enable strict validation
```

### 6. Integration Tests
**File:** `trading-data-services/tests/lmdb_integration_test.rs`

**Tests:**
- Read from actual LMDB (if available)
- Extract snapshots across time ranges
- Handle missing data gracefully
- Validate indicator calculations
- Test with llm-trader-data test database

---

## Data Mapping

### From LMDB to MarketStateSnapshot

| LMDB Field | Snapshot Field | Source DB |
|------------|---------------|-----------|
| ema_20 (3m) | ema_20 | indicators_3m |
| rsi_7 (3m) | rsi_7 | indicators_3m |
| rsi_14 (3m) | rsi_14 | indicators_3m |
| macd (3m) | macd | indicators_3m |
| ema_20 (4h) | ema_20_4h | indicators_4h |
| ema_50 (4h) | ema_50_4h | indicators_4h |
| atr_3 (4h) | atr_3_4h | indicators_4h |
| atr_14 (4h) | atr_14_4h | indicators_4h |
| macd (4h) | macd_4h_values[latest] | indicators_4h |
| rsi_14 (4h) | rsi_14_4h_values[latest] | indicators_4h |

### Time Series Construction
- **3m series** (last 10 points = 30 minutes):
  - Query indicators_3m for timestamps: `t-27m, t-24m, ..., t-3m, t`
  - Build vectors: mid_prices, ema_20_values, macd_values, rsi_7_values, rsi_14_values

- **4h series** (last 10 points = 40 hours):
  - Query indicators_4h for timestamps: `t-36h, t-32h, ..., t-4h, t`
  - Build vectors: macd_4h_values, rsi_14_4h_values

### Outcome Calculation
- Query future candles from candles_3m/candles_4h
- Calculate price changes at 15m, 1h, 4h, 24h horizons
- Track max runup/drawdown during period
- Note: Requires future data, so ingestion should lag by 24h

---

## Error Handling

### Missing Data Scenarios
1. **Symbol not found**: Skip and log warning
2. **Incomplete time series**: Use partial data if ≥ 50% available
3. **Missing indicators**: Skip snapshot, log error
4. **Future data unavailable**: Set outcomes to None
5. **LMDB corruption**: Fail fast with clear error message

### Validation Failures
- Log validation errors with context
- Continue processing other snapshots
- Report summary statistics at end

---

## Testing Strategy

### Unit Tests
- LMDB reader key/value parsing
- Time-range query logic
- Data validation rules
- Error handling paths

### Integration Tests (Requires LMDB)
1. **With llm-trader-data test DB:**
   - Run llm-trader-data tests to populate LMDB
   - Read and verify data in rag-ingest
   - Compare against expected values

2. **End-to-End:**
   - Ingest 7 days of LMDB data
   - Verify Qdrant upload
   - Check data completeness
   - Validate indicator ranges

### Compatibility Tests
- Schema version matching
- Cross-service data integrity
- Time synchronization
- Exchange compatibility (Bybit)

---

## Deployment Considerations

### Shared Storage
- Both services must access same LMDB path
- Docker: Use shared volume mount
- Bare metal: Shared filesystem path
- Kubernetes: PersistentVolumeClaim

### Example Docker Compose
```yaml
services:
  llm-trader-data:
    volumes:
      - trading-data:/shared/data/trading/lmdb

  llm-trader-rag:
    volumes:
      - trading-data:/shared/data/trading/lmdb:ro  # Read-only!

volumes:
  trading-data:
    driver: local
```

### Read-Only Access
- Important: rag-ingest must never write to LMDB
- Use read-only transactions only
- Validate no write operations in code

---

## Success Criteria

- [ ] LMDB reader successfully opens all 4 databases
- [ ] Extract complete snapshots with all required fields
- [ ] Time series data matches llm-trader-data calculations
- [ ] Validation detects and rejects bad data
- [ ] Integration tests pass with real LMDB data
- [ ] Performance: Extract 10K snapshots in <10 seconds
- [ ] Zero data corruption or write attempts
- [ ] Documentation complete and accurate

---

## Next Steps

1. Implement `LmdbReader` in Rust
2. Update `SnapshotExtractor` to use LMDB
3. Add data validation layer
4. Write comprehensive tests
5. Update CLI and configuration
6. Document usage and deployment

---

## References

- llm-trader-data: `src/storage/lmdb_writer.py`
- LMDB Rust crate: https://docs.rs/lmdb/latest/lmdb/
- Spec: `spec/LLM_BOT_RAG_IMPLEMENTATION.md` (Phase 6)
