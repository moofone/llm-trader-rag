use crate::types::TimestampMS;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Market state snapshot capturing all indicators, time series, and outcomes
/// for a specific point in time. This is the primary data structure for RAG.
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
    pub rsi_7: f64,  // 7-period RSI (Wilder's)
    pub rsi_14: f64, // 14-period RSI (Wilder's)
    pub macd: f64,   // MACD line only (EMA12 - EMA26)
    pub ema_20: f64, // 20-period EMA on 3m

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
    pub atr_3_4h: f64,  // Short-term volatility
    pub atr_14_4h: f64, // Standard volatility
    pub current_volume_4h: f64,
    pub avg_volume_4h: f64,
    pub macd_4h_values: Vec<f64>,   // Last 10 × 4h
    pub rsi_14_4h_values: Vec<f64>, // Last 10 × 4h

    // ═══════════════════════════════════════════════════
    // MARKET MICROSTRUCTURE (Futures-specific)
    // ═══════════════════════════════════════════════════
    pub open_interest_latest: f64,
    pub open_interest_avg_24h: f64,
    pub funding_rate: f64,      // Current perpetual funding rate (%)
    pub price_change_1h: f64,   // % change
    pub price_change_4h: f64,   // % change

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
    /// Create a new empty snapshot with defaults
    pub fn new(symbol: String, timestamp: TimestampMS, price: f64) -> Self {
        Self {
            symbol,
            timestamp,
            price,
            rsi_7: 0.0,
            rsi_14: 0.0,
            macd: 0.0,
            ema_20: 0.0,
            mid_prices: Vec::new(),
            ema_20_values: Vec::new(),
            macd_values: Vec::new(),
            rsi_7_values: Vec::new(),
            rsi_14_values: Vec::new(),
            ema_20_4h: 0.0,
            ema_50_4h: 0.0,
            atr_3_4h: 0.0,
            atr_14_4h: 0.0,
            current_volume_4h: 0.0,
            avg_volume_4h: 0.0,
            macd_4h_values: Vec::new(),
            rsi_14_4h_values: Vec::new(),
            open_interest_latest: 0.0,
            open_interest_avg_24h: 0.0,
            funding_rate: 0.0,
            price_change_1h: 0.0,
            price_change_4h: 0.0,
            outcome_15m: None,
            outcome_1h: None,
            outcome_4h: None,
            outcome_24h: None,
            max_drawdown_1h: None,
            max_runup_1h: None,
            hit_stop_loss: None,
            hit_take_profit: None,
        }
    }

    /// Calculate derived features from the snapshot data
    pub fn ema_ratio_20_50(&self) -> f64 {
        if self.ema_50_4h.abs() > 1e-10 {
            self.ema_20_4h / self.ema_50_4h
        } else {
            1.0
        }
    }

    /// Calculate OI delta percentage
    pub fn oi_delta_pct(&self) -> f64 {
        if self.open_interest_avg_24h.abs() > 1e-10 {
            ((self.open_interest_latest - self.open_interest_avg_24h)
                / self.open_interest_avg_24h)
                * 100.0
        } else {
            0.0
        }
    }

    /// Calculate slope from a series of values using simple linear regression
    pub fn calculate_slope(values: &[f64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }

        let n = values.len() as f64;
        let x_mean = (values.len() - 1) as f64 / 2.0;
        let y_mean = values.iter().sum::<f64>() / n;

        let numerator: f64 = values
            .iter()
            .enumerate()
            .map(|(i, &y)| (i as f64 - x_mean) * (y - y_mean))
            .sum();

        let denominator: f64 = values
            .iter()
            .enumerate()
            .map(|(i, _)| (i as f64 - x_mean).powi(2))
            .sum();

        if denominator.abs() < 1e-10 {
            0.0
        } else {
            numerator / denominator
        }
    }

    /// Calculate RSI slope from the time series
    pub fn rsi_7_slope(&self) -> f64 {
        Self::calculate_slope(&self.rsi_7_values)
    }

    /// Calculate MACD slope from the time series
    pub fn macd_slope(&self) -> f64 {
        Self::calculate_slope(&self.macd_values)
    }

    /// Calculate outcomes by looking at future price data
    /// Note: This is a placeholder. Real implementation would query LMDB for future prices.
    pub fn calculate_outcomes_from_future_prices(
        &mut self,
        price_15m: Option<f64>,
        price_1h: Option<f64>,
        price_4h: Option<f64>,
        price_24h: Option<f64>,
        prices_intraperiod_1h: Vec<f64>,
    ) -> Result<()> {
        let base_price = self.price;

        // Helper to calculate % change
        let calc_pct_change = |future_price: f64| -> f64 {
            ((future_price - base_price) / base_price) * 100.0
        };

        // Calculate outcomes
        self.outcome_15m = price_15m.map(calc_pct_change);
        self.outcome_1h = price_1h.map(calc_pct_change);
        self.outcome_4h = price_4h.map(calc_pct_change);
        self.outcome_24h = price_24h.map(calc_pct_change);

        // Calculate intra-period metrics from the 1h price series
        if !prices_intraperiod_1h.is_empty() {
            self.calculate_intraperiod_metrics(&prices_intraperiod_1h, base_price);
        }

        Ok(())
    }

    /// Calculate max runup, max drawdown, and stop/target hits
    fn calculate_intraperiod_metrics(&mut self, prices: &[f64], base_price: f64) {
        let mut max_runup = 0.0f64;
        let mut max_drawdown = 0.0f64;
        let mut hit_stop = false;
        let mut hit_tp = false;

        const STOP_LOSS_PCT: f64 = -2.0;
        const TAKE_PROFIT_PCT: f64 = 3.0;

        for &price in prices {
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

        self.max_runup_1h = Some(max_runup);
        self.max_drawdown_1h = Some(max_drawdown);
        self.hit_stop_loss = Some(hit_stop);
        self.hit_take_profit = Some(hit_tp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);
        assert_eq!(snapshot.symbol, "BTCUSDT");
        assert_eq!(snapshot.price, 50000.0);
        assert!(snapshot.outcome_1h.is_none());
    }

    #[test]
    fn test_calculate_slope() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let slope = MarketStateSnapshot::calculate_slope(&values);
        assert!((slope - 1.0).abs() < 0.01); // Should be ~1.0

        let flat = vec![5.0, 5.0, 5.0, 5.0];
        let slope_flat = MarketStateSnapshot::calculate_slope(&flat);
        assert_eq!(slope_flat, 0.0);
    }

    #[test]
    fn test_outcome_calculation() {
        let mut snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);

        snapshot
            .calculate_outcomes_from_future_prices(
                Some(50500.0), // +1%
                Some(51000.0), // +2%
                Some(49000.0), // -2%
                Some(52000.0), // +4%
                vec![50500.0, 51000.0, 50000.0, 49000.0],
            )
            .unwrap();

        assert_eq!(snapshot.outcome_15m, Some(1.0));
        assert_eq!(snapshot.outcome_1h, Some(2.0));
        assert_eq!(snapshot.outcome_4h, Some(-2.0));
        assert_eq!(snapshot.outcome_24h, Some(4.0));
        assert_eq!(snapshot.hit_stop_loss, Some(true));
    }

    #[test]
    fn test_derived_features() {
        let mut snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);
        snapshot.ema_20_4h = 50500.0;
        snapshot.ema_50_4h = 50000.0;

        let ratio = snapshot.ema_ratio_20_50();
        assert!((ratio - 1.01).abs() < 0.01);

        snapshot.open_interest_latest = 110.0;
        snapshot.open_interest_avg_24h = 100.0;
        let oi_delta = snapshot.oi_delta_pct();
        assert_eq!(oi_delta, 10.0);
    }
}
