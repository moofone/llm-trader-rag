use trading_core::MarketStateSnapshot;

/// Trait for converting market snapshots into natural language text for embeddings
pub trait SnapshotFormatter {
    /// Convert snapshot to detailed natural language format with semantic interpretation
    fn to_embedding_text(&self) -> String;

    /// Convert snapshot to simpler numerical format (faster to process)
    fn to_embedding_text_simple(&self) -> String;
}

impl SnapshotFormatter for MarketStateSnapshot {
    /// Detailed natural language format (more semantic info for embeddings)
    fn to_embedding_text(&self) -> String {
        let mut parts = Vec::new();

        // Trend indicators
        parts.push(format!(
            "RSI(7) is {:.1}, which is {}",
            self.rsi_7,
            interpret_rsi(self.rsi_7)
        ));
        parts.push(format!("RSI(14) is {:.1}", self.rsi_14));

        // MACD
        parts.push(format!("MACD is {:.2}", self.macd));
        let macd_slope = self.macd_slope();
        let macd_mom = if macd_slope > 0.0 {
            "rising"
        } else if macd_slope < 0.0 {
            "falling"
        } else {
            "flat"
        };
        parts.push(format!(
            "MACD momentum is {} (slope {:.3})",
            macd_mom, macd_slope
        ));

        // EMA trend
        let ema_ratio = self.ema_ratio_20_50();
        let trend = if ema_ratio > 1.005 {
            "strong uptrend"
        } else if ema_ratio < 0.995 {
            "strong downtrend"
        } else {
            "sideways"
        };
        parts.push(format!(
            "EMA(20)/EMA(50) ratio is {:.4}, indicating {}",
            ema_ratio, trend
        ));

        // Open Interest
        let oi_delta = self.oi_delta_pct();
        let oi_sentiment = if oi_delta > 5.0 {
            "rising significantly"
        } else if oi_delta < -5.0 {
            "dropping significantly"
        } else {
            "stable"
        };
        parts.push(format!(
            "Open interest is {} ({:+.1}% vs 24h average)",
            oi_sentiment, oi_delta
        ));

        // Funding
        let funding_sentiment = if self.funding_rate > 0.0005 {
            "highly positive (longs paying shorts)"
        } else if self.funding_rate < -0.0005 {
            "highly negative (shorts paying longs)"
        } else {
            "neutral"
        };
        parts.push(format!("Funding rate is {}", funding_sentiment));

        // Momentum
        let rsi_slope = self.rsi_7_slope();
        if rsi_slope.abs() > 2.0 {
            let direction = if rsi_slope > 0.0 {
                "accelerating up"
            } else {
                "accelerating down"
            };
            parts.push(format!("RSI momentum is {}", direction));
        }

        // Volatility context
        if self.atr_14_4h > 0.0 && self.atr_3_4h > 0.0 {
            let vol_state = if self.atr_3_4h > self.atr_14_4h * 1.5 {
                "elevated"
            } else {
                "normal"
            };
            parts.push(format!("Volatility is {}", vol_state));
        }

        // Price momentum
        if self.price_change_1h.abs() > 0.5 {
            parts.push(format!(
                "Price changed {:+.2}% in the last hour",
                self.price_change_1h
            ));
        }
        if self.price_change_4h.abs() > 1.0 {
            parts.push(format!(
                "Price changed {:+.2}% in the last 4 hours",
                self.price_change_4h
            ));
        }

        // Join all parts
        format!("Market state for {}: {}", self.symbol, parts.join(". "))
    }

    /// Simpler numerical format (faster to process)
    fn to_embedding_text_simple(&self) -> String {
        format!(
            "Symbol: {}, Price: {:.1}, RSI(7): {:.1}, RSI(14): {:.1}, MACD: {:.2}, \
             EMA Ratio 20/50: {:.4}, OI Delta: {:+.1}%, Funding: {:.6}, \
             ATR(14): {:.2}, Price Change 1h: {:+.2}%, Price Change 4h: {:+.2}%",
            self.symbol,
            self.price,
            self.rsi_7,
            self.rsi_14,
            self.macd,
            self.ema_ratio_20_50(),
            self.oi_delta_pct(),
            self.funding_rate,
            self.atr_14_4h,
            self.price_change_1h,
            self.price_change_4h
        )
    }
}

/// Interpret RSI value with descriptive text
fn interpret_rsi(rsi: f64) -> &'static str {
    match rsi {
        r if r >= 80.0 => "extremely overbought",
        r if r >= 70.0 => "overbought",
        r if r >= 60.0 => "bullish territory",
        r if r >= 40.0 => "neutral",
        r if r >= 30.0 => "bearish territory",
        r if r >= 20.0 => "oversold",
        _ => "extremely oversold",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_text_generation() {
        let mut snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);
        snapshot.rsi_7 = 75.0;
        snapshot.rsi_14 = 72.0;
        snapshot.macd = 100.0;
        snapshot.ema_20_4h = 50500.0;
        snapshot.ema_50_4h = 50000.0;
        snapshot.open_interest_latest = 110.0;
        snapshot.open_interest_avg_24h = 100.0;
        snapshot.funding_rate = 0.0001;

        let text = snapshot.to_embedding_text();
        assert!(text.contains("BTCUSDT"));
        assert!(text.contains("overbought"));
        assert!(text.contains("uptrend"));
    }

    #[test]
    fn test_simple_embedding_text() {
        let snapshot = MarketStateSnapshot::new("ETHUSDT".to_string(), 1000000, 3000.0);
        let text = snapshot.to_embedding_text_simple();
        assert!(text.contains("ETHUSDT"));
        assert!(text.contains("3000.0"));
    }

    #[test]
    fn test_rsi_interpretation() {
        assert_eq!(interpret_rsi(85.0), "extremely overbought");
        assert_eq!(interpret_rsi(75.0), "overbought");
        assert_eq!(interpret_rsi(65.0), "bullish territory");
        assert_eq!(interpret_rsi(50.0), "neutral");
        assert_eq!(interpret_rsi(35.0), "bearish territory");
        assert_eq!(interpret_rsi(25.0), "oversold");
        assert_eq!(interpret_rsi(15.0), "extremely oversold");
    }
}
