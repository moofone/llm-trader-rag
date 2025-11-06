use anyhow::{anyhow, Context, Result};
use lmdb::{Database, Environment, Transaction};
use serde_json::Value;
use std::path::Path;
use tracing;

/// LMDB reader for historical market data from llm-trader-data
///
/// Provides read-only access to the LMDB storage maintained by llm-trader-data.
/// Supports querying candles and indicators across multiple timeframes.
///
/// Database structure (from llm-trader-data):
/// - candles_3m: 3-minute OHLCV candles
/// - candles_4h: 4-hour OHLCV candles
/// - indicators_3m: 3-minute technical indicators
/// - indicators_4h: 4-hour technical indicators
///
/// Key format: {symbol}:{timestamp_ms}
/// Value format: JSON serialized dict
pub struct LmdbReader {
    env: Environment,
    db_candles_3m: Database,
    db_candles_4h: Database,
    db_indicators_3m: Database,
    db_indicators_4h: Database,
}

impl LmdbReader {
    /// Open LMDB environment in read-only mode
    ///
    /// # Arguments
    /// * `db_path` - Path to LMDB directory (shared with llm-trader-data)
    ///
    /// # Returns
    /// LMDB reader instance with all databases opened
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref();

        // Verify path exists
        if !db_path.exists() {
            return Err(anyhow!(
                "LMDB path does not exist: {}",
                db_path.display()
            ));
        }

        tracing::info!("Opening LMDB read-only at: {}", db_path.display());

        // Open environment in read-only mode
        let env = Environment::new()
            .set_max_dbs(10)
            .set_flags(lmdb::EnvironmentFlags::READ_ONLY)
            .open(db_path)
            .context("Failed to open LMDB environment")?;

        // Open all named databases
        let db_candles_3m = env
            .open_db(Some("candles_3m"))
            .context("Failed to open candles_3m database")?;

        let db_candles_4h = env
            .open_db(Some("candles_4h"))
            .context("Failed to open candles_4h database")?;

        let db_indicators_3m = env
            .open_db(Some("indicators_3m"))
            .context("Failed to open indicators_3m database")?;

        let db_indicators_4h = env
            .open_db(Some("indicators_4h"))
            .context("Failed to open indicators_4h database")?;

        tracing::info!("Successfully opened all 4 LMDB databases");

        Ok(Self {
            env,
            db_candles_3m,
            db_candles_4h,
            db_indicators_3m,
            db_indicators_4h,
        })
    }

    /// Generate LMDB key from symbol and timestamp
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// Key string in format: {symbol}:{timestamp_ms}
    fn make_key(symbol: &str, timestamp_ms: i64) -> String {
        format!("{}:{}", symbol, timestamp_ms)
    }

    /// Read indicators from specified database
    ///
    /// # Arguments
    /// * `db` - Database to query
    /// * `symbol` - Trading pair symbol
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// JSON value with indicators, or None if not found
    fn read_indicators(
        &self,
        db: Database,
        symbol: &str,
        timestamp_ms: i64,
    ) -> Result<Option<Value>> {
        let txn = self.env.begin_ro_txn().context("Failed to begin read transaction")?;
        let key = Self::make_key(symbol, timestamp_ms);

        match txn.get(db, &key) {
            Ok(bytes) => {
                let json: Value = serde_json::from_slice(bytes)
                    .context("Failed to deserialize indicator JSON")?;
                Ok(Some(json))
            }
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(anyhow!("LMDB read error: {}", e)),
        }
    }

    /// Read 3-minute indicators for a specific timestamp
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// JSON object with fields: ema_20, ema_50, macd, rsi_7, rsi_14, atr_14
    pub fn read_indicators_3m(&self, symbol: &str, timestamp_ms: i64) -> Result<Option<Value>> {
        self.read_indicators(self.db_indicators_3m, symbol, timestamp_ms)
    }

    /// Read 4-hour indicators for a specific timestamp
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "BTCUSDT")
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// JSON object with fields: ema_20, ema_50, macd, rsi_14, atr_3, atr_14
    pub fn read_indicators_4h(&self, symbol: &str, timestamp_ms: i64) -> Result<Option<Value>> {
        self.read_indicators(self.db_indicators_4h, symbol, timestamp_ms)
    }

    /// Read 3-minute candle data
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// JSON object with OHLCV data
    pub fn read_candles_3m(&self, symbol: &str, timestamp_ms: i64) -> Result<Option<Value>> {
        self.read_indicators(self.db_candles_3m, symbol, timestamp_ms)
    }

    /// Read 4-hour candle data
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol
    /// * `timestamp_ms` - Unix timestamp in milliseconds
    ///
    /// # Returns
    /// JSON object with OHLCV data
    pub fn read_candles_4h(&self, symbol: &str, timestamp_ms: i64) -> Result<Option<Value>> {
        self.read_indicators(self.db_candles_4h, symbol, timestamp_ms)
    }

    /// Read time series of 3-minute indicators
    ///
    /// Reads the last N data points for building time series vectors.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol
    /// * `end_timestamp_ms` - End timestamp (inclusive)
    /// * `interval_ms` - Interval between data points (e.g., 180_000 for 3m)
    /// * `count` - Number of data points to read
    ///
    /// # Returns
    /// Vector of (timestamp, indicators) tuples, ordered from oldest to newest
    pub fn read_indicators_3m_series(
        &self,
        symbol: &str,
        end_timestamp_ms: i64,
        interval_ms: i64,
        count: usize,
    ) -> Result<Vec<(i64, Value)>> {
        let mut results = Vec::with_capacity(count);

        for i in (0..count).rev() {
            let offset = i as i64 * interval_ms;
            let timestamp = end_timestamp_ms - offset;

            if let Some(data) = self.read_indicators_3m(symbol, timestamp)? {
                results.push((timestamp, data));
            } else {
                tracing::warn!(
                    "Missing 3m indicator data for {} at timestamp {}",
                    symbol,
                    timestamp
                );
            }
        }

        Ok(results)
    }

    /// Read time series of 4-hour indicators
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol
    /// * `end_timestamp_ms` - End timestamp (inclusive)
    /// * `interval_ms` - Interval between data points (e.g., 14_400_000 for 4h)
    /// * `count` - Number of data points to read
    ///
    /// # Returns
    /// Vector of (timestamp, indicators) tuples, ordered from oldest to newest
    pub fn read_indicators_4h_series(
        &self,
        symbol: &str,
        end_timestamp_ms: i64,
        interval_ms: i64,
        count: usize,
    ) -> Result<Vec<(i64, Value)>> {
        let mut results = Vec::with_capacity(count);

        for i in (0..count).rev() {
            let offset = i as i64 * interval_ms;
            let timestamp = end_timestamp_ms - offset;

            if let Some(data) = self.read_indicators_4h(symbol, timestamp)? {
                results.push((timestamp, data));
            } else {
                tracing::warn!(
                    "Missing 4h indicator data for {} at timestamp {}",
                    symbol,
                    timestamp
                );
            }
        }

        Ok(results)
    }

    /// Generate timestamps for a time range based on interval
    ///
    /// This generates expected timestamps and filters to those with data available.
    /// More efficient than scanning the entire database.
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol
    /// * `start_ms` - Start timestamp (inclusive)
    /// * `end_ms` - End timestamp (inclusive)
    /// * `interval_ms` - Interval in milliseconds (e.g., 180_000 for 3m)
    ///
    /// # Returns
    /// Vector of timestamps with available data
    pub fn query_timestamps_3m(
        &self,
        symbol: &str,
        start_ms: i64,
        end_ms: i64,
        interval_ms: i64,
    ) -> Result<Vec<i64>> {
        let mut timestamps = Vec::new();
        let mut current = start_ms;

        while current <= end_ms {
            // Check if data exists at this timestamp
            if self.read_indicators_3m(symbol, current)?.is_some() {
                timestamps.push(current);
            }
            current += interval_ms;
        }

        tracing::debug!(
            "Found {} timestamps for {} between {} and {} (interval: {}ms)",
            timestamps.len(),
            symbol,
            start_ms,
            end_ms,
            interval_ms
        );

        Ok(timestamps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_key() {
        let key = LmdbReader::make_key("BTCUSDT", 1730811225000);
        assert_eq!(key, "BTCUSDT:1730811225000");
    }

    #[test]
    fn test_make_key_different_symbol() {
        let key = LmdbReader::make_key("ETHUSDT", 1234567890000);
        assert_eq!(key, "ETHUSDT:1234567890000");
    }

    // Integration test - requires actual LMDB database
    #[test]
    #[ignore]
    fn test_open_lmdb() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb");
        assert!(reader.is_ok());
    }

    // Integration test - requires actual LMDB database with data
    #[test]
    #[ignore]
    fn test_read_indicators_3m() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").unwrap();
        let result = reader.read_indicators_3m("BTCUSDT", 1730811225000);
        // Should return Ok(Some(_)) or Ok(None) depending on data availability
        assert!(result.is_ok());
    }
}
