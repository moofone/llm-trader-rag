/// Indicator validation tests
///
/// Ensures that:
/// 1. Indicator data from LMDB has correct structure and ranges
/// 2. Indicators can be computed from the data
/// 3. Time series data is continuous and valid
/// 4. Cross-validation between different timeframes
use anyhow::Result;
use trading_data_services::{HistoricalSnapshotExtractor, LmdbReader};

#[cfg(test)]
mod indicator_structure_tests {
    use super::*;
    use serde_json::Value;

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_3m_indicators_structure() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let ts = 1730811225000;
        let indicators = reader.read_indicators_3m("BTCUSDT", ts)
            .expect("Failed to read indicators");

        if let Some(data) = indicators {
            // Verify required fields exist
            assert!(data.get("ema_20").is_some(), "Missing ema_20");
            assert!(data.get("ema_50").is_some(), "Missing ema_50");
            assert!(data.get("macd").is_some(), "Missing macd");
            assert!(data.get("rsi_7").is_some(), "Missing rsi_7");
            assert!(data.get("rsi_14").is_some(), "Missing rsi_14");
            assert!(data.get("atr_14").is_some(), "Missing atr_14");

            // Verify all values are numbers
            assert!(data["ema_20"].is_f64() || data["ema_20"].is_i64());
            assert!(data["ema_50"].is_f64() || data["ema_50"].is_i64());
            assert!(data["macd"].is_f64() || data["macd"].is_i64());
            assert!(data["rsi_7"].is_f64() || data["rsi_7"].is_i64());
            assert!(data["rsi_14"].is_f64() || data["rsi_14"].is_i64());
            assert!(data["atr_14"].is_f64() || data["atr_14"].is_i64());
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_4h_indicators_structure() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let ts = 1730811225000;
        let indicators = reader.read_indicators_4h("BTCUSDT", ts)
            .expect("Failed to read indicators");

        if let Some(data) = indicators {
            // Verify required fields exist
            assert!(data.get("ema_20").is_some(), "Missing ema_20");
            assert!(data.get("ema_50").is_some(), "Missing ema_50");
            assert!(data.get("macd").is_some(), "Missing macd");
            assert!(data.get("rsi_14").is_some(), "Missing rsi_14");
            assert!(data.get("atr_3").is_some(), "Missing atr_3");
            assert!(data.get("atr_14").is_some(), "Missing atr_14");

            // Verify all values are numbers
            assert!(data["ema_20"].is_f64() || data["ema_20"].is_i64());
            assert!(data["ema_50"].is_f64() || data["ema_50"].is_i64());
        }
    }

    #[test]
    fn test_indicator_field_extraction() {
        // Test the helper function for extracting f64 values
        let json = serde_json::json!({
            "ema_20": 50000.5,
            "rsi_14": 65.3,
            "macd": 125.7,
        });

        fn extract_f64(json: &Value, field: &str) -> Option<f64> {
            json.get(field).and_then(|v| v.as_f64())
        }

        assert_eq!(extract_f64(&json, "ema_20"), Some(50000.5));
        assert_eq!(extract_f64(&json, "rsi_14"), Some(65.3));
        assert_eq!(extract_f64(&json, "macd"), Some(125.7));
        assert_eq!(extract_f64(&json, "missing"), None);
    }
}

#[cfg(test)]
mod indicator_range_validation {
    use super::*;

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_rsi_range_from_lmdb() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        // Check multiple timestamps
        let base_ts = 1730811225000;
        let mut rsi_values = Vec::new();

        for i in 0..100 {
            let ts = base_ts + (i * 180000); // 3-minute intervals
            if let Ok(Some(data)) = reader.read_indicators_3m("BTCUSDT", ts) {
                if let Some(rsi_7) = data.get("rsi_7").and_then(|v| v.as_f64()) {
                    rsi_values.push(rsi_7);

                    // RSI must be in range [0, 100]
                    assert!(rsi_7 >= 0.0 && rsi_7 <= 100.0,
                        "RSI7 out of range at ts {}: {}", ts, rsi_7);
                    assert!(rsi_7.is_finite(), "RSI7 is not finite: {}", rsi_7);
                }

                if let Some(rsi_14) = data.get("rsi_14").and_then(|v| v.as_f64()) {
                    assert!(rsi_14 >= 0.0 && rsi_14 <= 100.0,
                        "RSI14 out of range at ts {}: {}", ts, rsi_14);
                    assert!(rsi_14.is_finite(), "RSI14 is not finite: {}", rsi_14);
                }
            }
        }

        if !rsi_values.is_empty() {
            println!("Validated {} RSI values", rsi_values.len());
            println!("RSI range: {:.2} to {:.2}",
                rsi_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
                rsi_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))
            );
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_ema_positive_from_lmdb() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let base_ts = 1730811225000;

        for i in 0..50 {
            let ts = base_ts + (i * 180000);
            if let Ok(Some(data)) = reader.read_indicators_3m("BTCUSDT", ts) {
                // EMA values should be positive (they're price-based)
                if let Some(ema_20) = data.get("ema_20").and_then(|v| v.as_f64()) {
                    assert!(ema_20 > 0.0, "EMA20 should be positive: {}", ema_20);
                    assert!(ema_20.is_finite(), "EMA20 is not finite");
                    // EMA for BTCUSDT should be in reasonable range
                    assert!(ema_20 > 100.0 && ema_20 < 200000.0,
                        "EMA20 out of reasonable range: {}", ema_20);
                }

                if let Some(ema_50) = data.get("ema_50").and_then(|v| v.as_f64()) {
                    assert!(ema_50 > 0.0, "EMA50 should be positive: {}", ema_50);
                    assert!(ema_50.is_finite(), "EMA50 is not finite");
                }
            }
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_atr_non_negative_from_lmdb() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let base_ts = 1730811225000;

        for i in 0..50 {
            let ts = base_ts + (i * 14400000); // 4-hour intervals
            if let Ok(Some(data)) = reader.read_indicators_4h("BTCUSDT", ts) {
                // ATR is always non-negative
                if let Some(atr_3) = data.get("atr_3").and_then(|v| v.as_f64()) {
                    assert!(atr_3 >= 0.0, "ATR3 should be non-negative: {}", atr_3);
                    assert!(atr_3.is_finite(), "ATR3 is not finite");
                }

                if let Some(atr_14) = data.get("atr_14").and_then(|v| v.as_f64()) {
                    assert!(atr_14 >= 0.0, "ATR14 should be non-negative: {}", atr_14);
                    assert!(atr_14.is_finite(), "ATR14 is not finite");
                }
            }
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_macd_finite_from_lmdb() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let base_ts = 1730811225000;
        let mut macd_values = Vec::new();

        for i in 0..100 {
            let ts = base_ts + (i * 180000);
            if let Ok(Some(data)) = reader.read_indicators_3m("BTCUSDT", ts) {
                if let Some(macd) = data.get("macd").and_then(|v| v.as_f64()) {
                    assert!(macd.is_finite(), "MACD is not finite: {}", macd);
                    macd_values.push(macd);
                }
            }
        }

        if !macd_values.is_empty() {
            println!("Validated {} MACD values", macd_values.len());
            println!("MACD range: {:.2} to {:.2}",
                macd_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
                macd_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))
            );
        }
    }
}

#[cfg(test)]
mod indicator_relationship_tests {
    use super::*;

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_ema_ordering() {
        // EMA20 should be closer to current price than EMA50
        // (assuming trending market, not always true but generally)
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let base_ts = 1730811225000;

        for i in 0..20 {
            let ts = base_ts + (i * 180000);
            if let Ok(Some(ind)) = reader.read_indicators_3m("BTCUSDT", ts) {
                if let Ok(Some(candle)) = reader.read_candles_3m("BTCUSDT", ts) {
                    let price = candle.get("close").and_then(|v| v.as_f64());
                    let ema20 = ind.get("ema_20").and_then(|v| v.as_f64());
                    let ema50 = ind.get("ema_50").and_then(|v| v.as_f64());

                    if let (Some(p), Some(e20), Some(e50)) = (price, ema20, ema50) {
                        // Both EMAs should be reasonably close to price
                        let price_to_ema20 = (p - e20).abs() / p;
                        let price_to_ema50 = (p - e50).abs() / p;

                        assert!(price_to_ema20 < 0.2, // Within 20%
                            "EMA20 too far from price: price={}, ema20={}", p, e20);
                        assert!(price_to_ema50 < 0.3, // Within 30%
                            "EMA50 too far from price: price={}, ema50={}", p, e50);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_atr_relationship() {
        // ATR14 is typically >= ATR3 (longer period = more smoothed)
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let base_ts = 1730811225000;
        let mut count = 0;
        let mut atr14_greater = 0;

        for i in 0..50 {
            let ts = base_ts + (i * 14400000);
            if let Ok(Some(data)) = reader.read_indicators_4h("BTCUSDT", ts) {
                let atr3 = data.get("atr_3").and_then(|v| v.as_f64());
                let atr14 = data.get("atr_14").and_then(|v| v.as_f64());

                if let (Some(a3), Some(a14)) = (atr3, atr14) {
                    count += 1;
                    // ATR values should be in similar range
                    let ratio = a14 / a3;
                    assert!(ratio > 0.1 && ratio < 10.0,
                        "ATR ratio out of reasonable range: atr3={}, atr14={}", a3, a14);

                    if a14 >= a3 {
                        atr14_greater += 1;
                    }
                }
            }
        }

        if count > 0 {
            println!("ATR14 >= ATR3 in {} out of {} cases ({:.1}%)",
                atr14_greater, count, (atr14_greater as f64 / count as f64) * 100.0);
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_rsi_correlation_between_periods() {
        // RSI7 and RSI14 should generally move together
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let base_ts = 1730811225000;
        let mut pairs = Vec::new();

        for i in 0..100 {
            let ts = base_ts + (i * 180000);
            if let Ok(Some(data)) = reader.read_indicators_3m("BTCUSDT", ts) {
                let rsi7 = data.get("rsi_7").and_then(|v| v.as_f64());
                let rsi14 = data.get("rsi_14").and_then(|v| v.as_f64());

                if let (Some(r7), Some(r14)) = (rsi7, rsi14) {
                    // RSI values should be in same general zone
                    let diff = (r7 - r14).abs();
                    assert!(diff < 50.0,
                        "RSI7 and RSI14 too different: rsi7={}, rsi14={}", r7, r14);
                    pairs.push((r7, r14));
                }
            }
        }

        if pairs.len() > 10 {
            // Calculate correlation (simplified)
            let mean_diff: f64 = pairs.iter()
                .map(|(r7, r14)| (r7 - r14).abs())
                .sum::<f64>() / pairs.len() as f64;

            println!("RSI7/RSI14 pairs: {}, average diff: {:.2}", pairs.len(), mean_diff);
            assert!(mean_diff < 20.0, "RSI7 and RSI14 should correlate reasonably");
        }
    }
}

#[cfg(test)]
mod time_series_continuity_tests {
    use super::*;

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_time_series_no_gaps() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let end_ts = 1730811225000;
        let interval = 180000; // 3 minutes
        let count = 10;

        let series = reader.read_indicators_3m_series("BTCUSDT", end_ts, interval, count)
            .expect("Failed to read time series");

        if series.len() >= 2 {
            // Check timestamps are evenly spaced
            for i in 1..series.len() {
                let ts_diff = series[i].0 - series[i-1].0;
                assert_eq!(ts_diff, interval,
                    "Time series gap detected: expected {}, got {}", interval, ts_diff);
            }
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_time_series_ordering() {
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let end_ts = 1730811225000;
        let series = reader.read_indicators_3m_series("BTCUSDT", end_ts, 180000, 10)
            .expect("Failed to read time series");

        // Timestamps should be in ascending order
        for i in 1..series.len() {
            assert!(series[i].0 > series[i-1].0,
                "Time series not ordered: ts[{}]={}, ts[{}]={}",
                i-1, series[i-1].0, i, series[i].0);
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_indicator_smoothness() {
        // Indicators should change gradually, not jump wildly
        let reader = LmdbReader::new("/shared/data/trading/lmdb").expect("Failed to open LMDB");

        let end_ts = 1730811225000;
        let series = reader.read_indicators_3m_series("BTCUSDT", end_ts, 180000, 20)
            .expect("Failed to read time series");

        if series.len() >= 2 {
            for i in 1..series.len() {
                if let (Some(rsi_prev), Some(rsi_curr)) = (
                    series[i-1].1.get("rsi_14").and_then(|v| v.as_f64()),
                    series[i].1.get("rsi_14").and_then(|v| v.as_f64())
                ) {
                    let change = (rsi_curr - rsi_prev).abs();
                    // RSI shouldn't jump more than 30 points in 3 minutes
                    assert!(change < 30.0,
                        "RSI changed too much: from {:.2} to {:.2} (change: {:.2})",
                        rsi_prev, rsi_curr, change);
                }

                if let (Some(ema_prev), Some(ema_curr)) = (
                    series[i-1].1.get("ema_20").and_then(|v| v.as_f64()),
                    series[i].1.get("ema_20").and_then(|v| v.as_f64())
                ) {
                    let pct_change = ((ema_curr - ema_prev) / ema_prev).abs() * 100.0;
                    // EMA shouldn't change more than 5% in 3 minutes
                    assert!(pct_change < 5.0,
                        "EMA changed too much: from {:.2} to {:.2} ({:.2}%)",
                        ema_prev, ema_curr, pct_change);
                }
            }
        }
    }
}

#[cfg(test)]
mod snapshot_indicator_validation {
    use super::*;

    #[test]
    fn test_mock_snapshot_indicators() {
        let extractor = HistoricalSnapshotExtractor::new();
        let snapshots = extractor.extract_snapshots("BTCUSDT", 1000000000, 1001000000, 15)
            .expect("Failed to extract snapshots");

        for snapshot in snapshots {
            // Validate RSI ranges
            assert!(snapshot.rsi_7 >= 0.0 && snapshot.rsi_7 <= 100.0);
            assert!(snapshot.rsi_14 >= 0.0 && snapshot.rsi_14 <= 100.0);

            // Validate positive values
            assert!(snapshot.price > 0.0);
            assert!(snapshot.ema_20 > 0.0);
            assert!(snapshot.ema_20_4h > 0.0);
            assert!(snapshot.ema_50_4h > 0.0);

            // Validate ATR
            assert!(snapshot.atr_3_4h >= 0.0);
            assert!(snapshot.atr_14_4h >= 0.0);

            // Validate finite values
            assert!(snapshot.macd.is_finite());
            assert!(snapshot.price.is_finite());

            // Validate time series
            assert_eq!(snapshot.mid_prices.len(), 10);
            assert_eq!(snapshot.rsi_7_values.len(), 10);
            assert_eq!(snapshot.rsi_14_values.len(), 10);

            for &rsi in &snapshot.rsi_7_values {
                assert!(rsi >= 0.0 && rsi <= 100.0);
            }

            for &rsi in &snapshot.rsi_14_values {
                assert!(rsi >= 0.0 && rsi <= 100.0);
            }

            for &price in &snapshot.mid_prices {
                assert!(price > 0.0 && price.is_finite());
            }
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_lmdb_snapshot_indicators() {
        let extractor = HistoricalSnapshotExtractor::with_lmdb("/shared/data/trading/lmdb")
            .expect("Failed to create LMDB extractor");

        let start = 1730811225000;
        let end = start + 3600000; // 1 hour

        let snapshots = extractor.extract_snapshots("BTCUSDT", start, end, 15)
            .expect("Failed to extract snapshots");

        if snapshots.is_empty() {
            println!("Warning: No snapshots extracted from LMDB");
            return;
        }

        for snapshot in &snapshots {
            // Same validations as mock data
            assert!(snapshot.rsi_7 >= 0.0 && snapshot.rsi_7 <= 100.0,
                "Invalid RSI7: {}", snapshot.rsi_7);
            assert!(snapshot.rsi_14 >= 0.0 && snapshot.rsi_14 <= 100.0,
                "Invalid RSI14: {}", snapshot.rsi_14);

            assert!(snapshot.price > 0.0, "Invalid price: {}", snapshot.price);
            assert!(snapshot.ema_20 > 0.0, "Invalid EMA20: {}", snapshot.ema_20);

            assert!(snapshot.atr_3_4h >= 0.0, "Invalid ATR3: {}", snapshot.atr_3_4h);
            assert!(snapshot.atr_14_4h >= 0.0, "Invalid ATR14: {}", snapshot.atr_14_4h);

            assert!(snapshot.macd.is_finite(), "MACD not finite: {}", snapshot.macd);

            // Validate time series data
            assert!(!snapshot.mid_prices.is_empty(), "Empty mid_prices");
            assert!(!snapshot.rsi_7_values.is_empty(), "Empty rsi_7_values");

            for &val in &snapshot.mid_prices {
                assert!(val > 0.0 && val.is_finite(), "Invalid mid_price: {}", val);
            }
        }

        println!("Validated {} snapshots from LMDB", snapshots.len());
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_snapshot_indicator_completeness() {
        // Verify that all expected indicator fields are populated
        let extractor = HistoricalSnapshotExtractor::with_lmdb("/shared/data/trading/lmdb")
            .expect("Failed to create LMDB extractor");

        let start = 1730811225000;
        let end = start + 900000; // 15 minutes

        let snapshots = extractor.extract_snapshots("BTCUSDT", start, end, 15)
            .expect("Failed to extract snapshots");

        for snapshot in snapshots {
            // Check all indicator fields are populated (not default 0.0)
            assert_ne!(snapshot.rsi_7, 0.0, "RSI7 is zero");
            assert_ne!(snapshot.rsi_14, 0.0, "RSI14 is zero");
            assert_ne!(snapshot.ema_20, 0.0, "EMA20 is zero");
            assert_ne!(snapshot.ema_20_4h, 0.0, "EMA20_4h is zero");
            assert_ne!(snapshot.ema_50_4h, 0.0, "EMA50_4h is zero");

            // ATR can be zero in theory but unlikely
            assert_ne!(snapshot.atr_3_4h, 0.0, "ATR3 is zero");
            assert_ne!(snapshot.atr_14_4h, 0.0, "ATR14 is zero");

            // Time series should be populated
            assert!(snapshot.mid_prices.iter().any(|&p| p != 0.0), "All mid_prices are zero");
            assert!(snapshot.ema_20_values.iter().any(|&e| e != 0.0), "All EMA20 values are zero");
        }
    }
}
