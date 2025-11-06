use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use trading_core::{MarketStateSnapshot, TimestampMS};
use tracing;

use super::lmdb_reader::LmdbReader;

/// Data source for snapshot extraction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DataSource {
    /// Use LMDB storage (real historical data from llm-trader-data)
    Lmdb,
    /// Use mock data generator (for testing)
    Mock,
}

/// Extracts historical market snapshots from LMDB storage or mock data
pub struct HistoricalSnapshotExtractor {
    data_source: DataSource,
    lmdb_reader: Option<LmdbReader>,
}

impl HistoricalSnapshotExtractor {
    /// Create a new snapshot extractor with mock data
    pub fn new() -> Self {
        Self {
            data_source: DataSource::Mock,
            lmdb_reader: None,
        }
    }

    /// Create a snapshot extractor with LMDB backend
    ///
    /// # Arguments
    /// * `lmdb_path` - Path to LMDB directory (shared with llm-trader-data)
    ///
    /// # Returns
    /// Extractor configured to read from LMDB
    pub fn with_lmdb(lmdb_path: &str) -> Result<Self> {
        let lmdb_reader = LmdbReader::new(lmdb_path)
            .context("Failed to initialize LMDB reader")?;

        tracing::info!("SnapshotExtractor initialized with LMDB backend at {}", lmdb_path);

        Ok(Self {
            data_source: DataSource::Lmdb,
            lmdb_reader: Some(lmdb_reader),
        })
    }

    /// Extract snapshots for a symbol in a time range
    ///
    /// # Arguments
    /// * `symbol` - Trading symbol (e.g., "BTCUSDT")
    /// * `start_timestamp` - Start time in milliseconds
    /// * `end_timestamp` - End time in milliseconds
    /// * `interval_minutes` - Snapshot frequency (e.g., 15)
    ///
    /// # Returns
    /// Vector of market snapshots with complete indicator data
    pub fn extract_snapshots(
        &self,
        symbol: &str,
        start_timestamp: TimestampMS,
        end_timestamp: TimestampMS,
        interval_minutes: u64,
    ) -> Result<Vec<MarketStateSnapshot>> {
        match self.data_source {
            DataSource::Lmdb => {
                self.extract_from_lmdb(symbol, start_timestamp, end_timestamp, interval_minutes)
            }
            DataSource::Mock => {
                self.extract_mock_snapshots(symbol, start_timestamp, end_timestamp, interval_minutes)
            }
        }
    }

    /// Extract snapshots from LMDB storage
    fn extract_from_lmdb(
        &self,
        symbol: &str,
        start_timestamp: TimestampMS,
        end_timestamp: TimestampMS,
        interval_minutes: u64,
    ) -> Result<Vec<MarketStateSnapshot>> {
        let lmdb = self.lmdb_reader.as_ref()
            .ok_or_else(|| anyhow!("LMDB reader not initialized"))?;

        let mut snapshots = Vec::new();
        let interval_ms = (interval_minutes * 60_000) as i64;
        let mut current_ts = start_timestamp as i64;
        let end_ts = end_timestamp as i64;

        let mut success_count = 0;
        let mut skip_count = 0;

        while current_ts < end_ts {
            match self.build_snapshot_from_lmdb(lmdb, symbol, current_ts) {
                Ok(snapshot) => {
                    snapshots.push(snapshot);
                    success_count += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to build snapshot for {} at {}: {}",
                        symbol,
                        current_ts,
                        e
                    );
                    skip_count += 1;
                }
            }

            current_ts += interval_ms;
        }

        tracing::info!(
            "Extracted {} snapshots for {} from {} to {} ({} skipped due to missing data)",
            success_count,
            symbol,
            start_timestamp,
            end_timestamp,
            skip_count
        );

        Ok(snapshots)
    }

    /// Build a complete snapshot from LMDB data
    fn build_snapshot_from_lmdb(
        &self,
        lmdb: &LmdbReader,
        symbol: &str,
        timestamp: i64,
    ) -> Result<MarketStateSnapshot> {
        // Read 3-minute indicators (current point)
        let indicators_3m = lmdb.read_indicators_3m(symbol, timestamp)?
            .ok_or_else(|| anyhow!("Missing 3m indicators for {} at {}", symbol, timestamp))?;

        // Read 4-hour indicators (current point)
        let indicators_4h = lmdb.read_indicators_4h(symbol, timestamp)?
            .ok_or_else(|| anyhow!("Missing 4h indicators for {} at {}", symbol, timestamp))?;

        // Read candle for price data
        let candle_3m = lmdb.read_candles_3m(symbol, timestamp)?
            .ok_or_else(|| anyhow!("Missing 3m candle for {} at {}", symbol, timestamp))?;

        // Extract price from candle
        let price = candle_3m.get("close")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow!("Missing close price in candle"))?;

        // Create snapshot
        let mut snapshot = MarketStateSnapshot::new(
            symbol.to_string(),
            timestamp as TimestampMS,
            price
        );

        // Fill 3-minute indicators
        snapshot.rsi_7 = Self::extract_f64(&indicators_3m, "rsi_7")?;
        snapshot.rsi_14 = Self::extract_f64(&indicators_3m, "rsi_14")?;
        snapshot.macd = Self::extract_f64(&indicators_3m, "macd")?;
        snapshot.ema_20 = Self::extract_f64(&indicators_3m, "ema_20")?;

        // Fill 4-hour indicators
        snapshot.ema_20_4h = Self::extract_f64(&indicators_4h, "ema_20")?;
        snapshot.ema_50_4h = Self::extract_f64(&indicators_4h, "ema_50")?;
        snapshot.atr_3_4h = Self::extract_f64(&indicators_4h, "atr_3")?;
        snapshot.atr_14_4h = Self::extract_f64(&indicators_4h, "atr_14")?;

        // Read time series data (last 10 points)
        self.fill_time_series_3m(lmdb, symbol, timestamp, &mut snapshot)?;
        self.fill_time_series_4h(lmdb, symbol, timestamp, &mut snapshot)?;

        // TODO: Add derivatives data (OI, funding rate) when available in LMDB
        // For now, use placeholder values
        snapshot.open_interest_latest = 0.0;
        snapshot.open_interest_avg_24h = 0.0;
        snapshot.funding_rate = 0.0;
        snapshot.price_change_1h = 0.0;
        snapshot.price_change_4h = 0.0;

        // TODO: Calculate outcomes from future data
        // This requires querying future candles and calculating price changes
        snapshot.outcome_15m = None;
        snapshot.outcome_1h = None;
        snapshot.outcome_4h = None;
        snapshot.outcome_24h = None;

        Ok(snapshot)
    }

    /// Fill 3-minute time series data
    fn fill_time_series_3m(
        &self,
        lmdb: &LmdbReader,
        symbol: &str,
        end_timestamp: i64,
        snapshot: &mut MarketStateSnapshot,
    ) -> Result<()> {
        let interval_3m = 180_000; // 3 minutes in ms
        let series = lmdb.read_indicators_3m_series(symbol, end_timestamp, interval_3m, 10)?;

        if series.is_empty() {
            return Err(anyhow!("No 3m time series data available"));
        }

        // Extract vectors from series
        snapshot.ema_20_values = series.iter()
            .filter_map(|(_, data)| data.get("ema_20").and_then(|v| v.as_f64()))
            .collect();

        snapshot.macd_values = series.iter()
            .filter_map(|(_, data)| data.get("macd").and_then(|v| v.as_f64()))
            .collect();

        snapshot.rsi_7_values = series.iter()
            .filter_map(|(_, data)| data.get("rsi_7").and_then(|v| v.as_f64()))
            .collect();

        snapshot.rsi_14_values = series.iter()
            .filter_map(|(_, data)| data.get("rsi_14").and_then(|v| v.as_f64()))
            .collect();

        // Fill mid_prices from candles
        let candles: Result<Vec<_>> = series.iter()
            .map(|(ts, _)| {
                lmdb.read_candles_3m(symbol, *ts)?
                    .and_then(|c| c.get("close").and_then(|v| v.as_f64()))
                    .ok_or_else(|| anyhow!("Missing candle close price"))
            })
            .collect();

        snapshot.mid_prices = candles?;

        Ok(())
    }

    /// Fill 4-hour time series data
    fn fill_time_series_4h(
        &self,
        lmdb: &LmdbReader,
        symbol: &str,
        end_timestamp: i64,
        snapshot: &mut MarketStateSnapshot,
    ) -> Result<()> {
        let interval_4h = 14_400_000; // 4 hours in ms
        let series = lmdb.read_indicators_4h_series(symbol, end_timestamp, interval_4h, 10)?;

        if series.is_empty() {
            return Err(anyhow!("No 4h time series data available"));
        }

        snapshot.macd_4h_values = series.iter()
            .filter_map(|(_, data)| data.get("macd").and_then(|v| v.as_f64()))
            .collect();

        snapshot.rsi_14_4h_values = series.iter()
            .filter_map(|(_, data)| data.get("rsi_14").and_then(|v| v.as_f64()))
            .collect();

        Ok(())
    }

    /// Extract f64 value from JSON with error handling
    fn extract_f64(json: &Value, field: &str) -> Result<f64> {
        json.get(field)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", field))
    }

    /// Extract snapshots using mock data generator
    fn extract_mock_snapshots(
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
            let snapshot = self.create_mock_snapshot(symbol, current_ts)?;
            snapshots.push(snapshot);
            current_ts += interval_ms;
        }

        tracing::info!(
            "Extracted {} snapshots for {} from {} to {} (using mock data)",
            snapshots.len(),
            symbol,
            start_timestamp,
            end_timestamp
        );

        Ok(snapshots)
    }

    /// Create a mock snapshot for testing
    fn create_mock_snapshot(&self, symbol: &str, timestamp: TimestampMS) -> Result<MarketStateSnapshot> {
        use std::f64::consts::PI;

        // Create deterministic but varying mock data based on timestamp
        let t = (timestamp as f64) / 1000000.0;
        let base_price = 50000.0 + (t * PI).sin() * 5000.0;

        let mut snapshot = MarketStateSnapshot::new(symbol.to_string(), timestamp, base_price);

        // Mock indicators with some variation
        snapshot.rsi_7 = 50.0 + (t * 2.0 * PI).sin() * 30.0;
        snapshot.rsi_14 = 50.0 + (t * 1.5 * PI).sin() * 25.0;
        snapshot.macd = (t * PI).sin() * 100.0;
        snapshot.ema_20 = base_price * 0.99;
        snapshot.ema_20_4h = base_price * 0.98;
        snapshot.ema_50_4h = base_price * 0.97;
        snapshot.atr_3_4h = 200.0;
        snapshot.atr_14_4h = 250.0;
        snapshot.current_volume_4h = 1000000.0;
        snapshot.avg_volume_4h = 900000.0;
        snapshot.open_interest_latest = 100000.0 + (t * PI).sin() * 10000.0;
        snapshot.open_interest_avg_24h = 100000.0;
        snapshot.funding_rate = (t * PI).sin() * 0.0002;
        snapshot.price_change_1h = (t * PI).sin() * 2.0;
        snapshot.price_change_4h = (t * 0.5 * PI).sin() * 4.0;

        // Mock time series data
        snapshot.mid_prices = vec![base_price; 10];
        snapshot.ema_20_values = vec![base_price * 0.99; 10];
        snapshot.macd_values = vec![snapshot.macd; 10];
        snapshot.rsi_7_values = vec![snapshot.rsi_7; 10];
        snapshot.rsi_14_values = vec![snapshot.rsi_14; 10];
        snapshot.macd_4h_values = vec![snapshot.macd; 10];
        snapshot.rsi_14_4h_values = vec![snapshot.rsi_14; 10];

        // Mock outcomes (simulate future price movement)
        let future_change = (t * PI * 3.0).sin() * 2.0; // Random-ish % change
        snapshot.outcome_15m = Some(future_change * 0.25);
        snapshot.outcome_1h = Some(future_change * 0.5);
        snapshot.outcome_4h = Some(future_change);
        snapshot.outcome_24h = Some(future_change * 1.5);

        // Mock intraperiod metrics
        snapshot.max_runup_1h = Some(future_change.abs() * 1.2);
        snapshot.max_drawdown_1h = Some(-future_change.abs() * 0.8);
        snapshot.hit_stop_loss = Some(future_change < -1.5);
        snapshot.hit_take_profit = Some(future_change > 2.5);

        Ok(snapshot)
    }
}

impl Default for HistoricalSnapshotExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mock_snapshots() {
        let extractor = HistoricalSnapshotExtractor::new();
        let start = 1000000000;
        let end = 1000000000 + 60 * 60 * 1000; // 1 hour
        let snapshots = extractor
            .extract_snapshots("BTCUSDT", start, end, 15)
            .unwrap();

        // Should get 4 snapshots (0, 15, 30, 45 minutes)
        assert_eq!(snapshots.len(), 4);
        assert_eq!(snapshots[0].symbol, "BTCUSDT");
    }

    #[test]
    fn test_data_source_selection() {
        let mock_extractor = HistoricalSnapshotExtractor::new();
        assert_eq!(mock_extractor.data_source, DataSource::Mock);
    }

    // Integration test - requires actual LMDB database
    #[test]
    #[ignore]
    fn test_extract_from_lmdb() {
        let extractor = HistoricalSnapshotExtractor::with_lmdb("/shared/data/trading/lmdb")
            .expect("Failed to create LMDB extractor");

        assert_eq!(extractor.data_source, DataSource::Lmdb);

        // Try to extract some snapshots
        let start = 1730811225000; // Example timestamp
        let end = start + 3600000; // 1 hour later
        let snapshots = extractor.extract_snapshots("BTCUSDT", start, end, 15);

        // Should either succeed with data or fail gracefully
        match snapshots {
            Ok(data) => println!("Extracted {} snapshots from LMDB", data.len()),
            Err(e) => println!("Expected - no data available: {}", e),
        }
    }
}
