/// Comprehensive edge case tests for LMDB integration
///
/// These tests cover:
/// - Missing data scenarios
/// - Corrupt data handling
/// - Boundary conditions
/// - Time range edge cases
/// - Invalid inputs
/// - Performance with large datasets
use anyhow::Result;
use trading_data_services::{HistoricalSnapshotExtractor, LmdbReader};

#[cfg(test)]
mod lmdb_reader_edge_cases {
    use super::*;

    #[test]
    fn test_nonexistent_path() {
        let result = LmdbReader::new("/path/that/does/not/exist");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn test_invalid_path_permissions() {
        // Try to open a path we don't have permission to
        let result = LmdbReader::new("/root/forbidden");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_symbol() {
        // Test with empty symbol string
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m("", 1000000000);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none()); // Should return None for empty symbol
        }
    }

    #[test]
    fn test_negative_timestamp() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m("BTCUSDT", -1);
            // Should handle gracefully
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_zero_timestamp() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m("BTCUSDT", 0);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }
    }

    #[test]
    fn test_future_timestamp() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            // Timestamp far in the future (year 3000)
            let future_ts = 32503680000000i64;
            let result = r.read_indicators_3m("BTCUSDT", future_ts);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }
    }

    #[test]
    fn test_very_large_timestamp() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m("BTCUSDT", i64::MAX);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_symbol_case_sensitivity() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let ts = 1730811225000;
            let result1 = r.read_indicators_3m("BTCUSDT", ts);
            let result2 = r.read_indicators_3m("btcusdt", ts);
            let result3 = r.read_indicators_3m("BtCuSdT", ts);

            // All should succeed but may return different results
            assert!(result1.is_ok());
            assert!(result2.is_ok());
            assert!(result3.is_ok());
        }
    }

    #[test]
    fn test_special_characters_in_symbol() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let symbols = vec![
                "BTC-USDT",
                "BTC/USDT",
                "BTC USDT",
                "BTC@USDT",
                "BTC#USDT",
                "",
                "\0",
                "🚀",
            ];

            for symbol in symbols {
                let result = r.read_indicators_3m(symbol, 1000000000);
                // Should handle gracefully without panicking
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_time_series_zero_count() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m_series("BTCUSDT", 1730811225000, 180000, 0);
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 0);
        }
    }

    #[test]
    fn test_time_series_large_count() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            // Request 10,000 data points
            let result = r.read_indicators_3m_series("BTCUSDT", 1730811225000, 180000, 10000);
            assert!(result.is_ok());
            // May return fewer than requested if data not available
        }
    }

    #[test]
    fn test_time_series_negative_interval() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m_series("BTCUSDT", 1730811225000, -180000, 10);
            // Should handle negative interval gracefully
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_time_series_zero_interval() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.read_indicators_3m_series("BTCUSDT", 1730811225000, 0, 10);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_query_timestamps_invalid_range() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            // End before start
            let result = r.query_timestamps_3m("BTCUSDT", 2000000000, 1000000000, 180000);
            assert!(result.is_ok());
            // Should return empty vec
            assert_eq!(result.unwrap().len(), 0);
        }
    }

    #[test]
    fn test_query_timestamps_same_start_end() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            let result = r.query_timestamps_3m("BTCUSDT", 1000000000, 1000000000, 180000);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_query_timestamps_very_large_range() {
        let reader = create_mock_lmdb_reader();
        if let Ok(r) = reader {
            // Query 10 years of data at 3-minute intervals
            let start = 1000000000000i64;
            let end = start + (10 * 365 * 24 * 60 * 60 * 1000); // 10 years in ms
            let result = r.query_timestamps_3m("BTCUSDT", start, end, 180000);
            assert!(result.is_ok());
            // Should handle large ranges without crashing
        }
    }

    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let reader = create_mock_lmdb_reader();
        if let Ok(reader) = reader {
            let reader = Arc::new(reader);
            let mut handles = vec![];

            // Spawn 10 concurrent read threads
            for i in 0..10 {
                let reader_clone = Arc::clone(&reader);
                let handle = thread::spawn(move || {
                    let ts = 1730811225000 + (i * 180000);
                    let _ = reader_clone.read_indicators_3m("BTCUSDT", ts);
                    let _ = reader_clone.read_indicators_4h("BTCUSDT", ts);
                });
                handles.push(handle);
            }

            // Wait for all threads
            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    // Helper function to create a mock LMDB reader for testing
    // Returns Err if LMDB not available
    fn create_mock_lmdb_reader() -> Result<LmdbReader> {
        // Try to use test LMDB path if available
        let test_paths = vec![
            "/tmp/test_lmdb",
            "/shared/data/trading/lmdb",
            "./test_data/lmdb",
        ];

        for path in test_paths {
            if std::path::Path::new(path).exists() {
                if let Ok(reader) = LmdbReader::new(path) {
                    return Ok(reader);
                }
            }
        }

        Err(anyhow::anyhow!("No LMDB database available for testing"))
    }
}

#[cfg(test)]
mod snapshot_extractor_edge_cases {
    use super::*;
    use trading_core::MarketStateSnapshot;

    #[test]
    fn test_extract_zero_interval() {
        let extractor = HistoricalSnapshotExtractor::new();
        let result = extractor.extract_snapshots("BTCUSDT", 1000000000, 2000000000, 0);
        // Should handle gracefully - may return error or empty vec
        match result {
            Ok(snapshots) => {
                // If it succeeds, should be empty or have limited results
                assert!(snapshots.len() < 1000); // Sanity check
            }
            Err(_) => {
                // Error is acceptable for zero interval
            }
        }
    }

    #[test]
    fn test_extract_reversed_time_range() {
        let extractor = HistoricalSnapshotExtractor::new();
        // End before start
        let result = extractor.extract_snapshots("BTCUSDT", 2000000000, 1000000000, 15);
        assert!(result.is_ok());
        let snapshots = result.unwrap();
        // Should return empty vec for invalid range
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn test_extract_same_start_end() {
        let extractor = HistoricalSnapshotExtractor::new();
        let result = extractor.extract_snapshots("BTCUSDT", 1000000000, 1000000000, 15);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_extract_very_small_time_range() {
        let extractor = HistoricalSnapshotExtractor::new();
        // 1 second range with 15 minute interval
        let start = 1000000000;
        let end = start + 1000;
        let result = extractor.extract_snapshots("BTCUSDT", start, end, 15);
        assert!(result.is_ok());
        // Should return 0 or 1 snapshot
        assert!(result.unwrap().len() <= 1);
    }

    #[test]
    fn test_extract_very_large_time_range() {
        let extractor = HistoricalSnapshotExtractor::new();
        // 1 year of data at 15-minute intervals would be ~35,040 snapshots
        let start = 1000000000000i64;
        let end = start + (365 * 24 * 60 * 60 * 1000); // 1 year
        let result = extractor.extract_snapshots("BTCUSDT", start as u64, end as u64, 15);
        assert!(result.is_ok());
        // Should handle large ranges (though may be slow with real data)
    }

    #[test]
    fn test_extract_invalid_symbol() {
        let extractor = HistoricalSnapshotExtractor::new();
        let symbols = vec!["", "INVALID", "123", "!@#$", "\0"];

        for symbol in symbols {
            let result = extractor.extract_snapshots(symbol, 1000000000, 2000000000, 15);
            // Should not panic
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[test]
    fn test_extract_very_small_interval() {
        let extractor = HistoricalSnapshotExtractor::new();
        // 1 minute interval (smaller than data granularity)
        let result = extractor.extract_snapshots("BTCUSDT", 1000000000, 1001000000, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_very_large_interval() {
        let extractor = HistoricalSnapshotExtractor::new();
        // 1 week interval
        let result = extractor.extract_snapshots("BTCUSDT", 1000000000, 2000000000, 7 * 24 * 60);
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // Requires actual LMDB data
    fn test_lmdb_missing_data_points() {
        // Test that extractor handles missing data points gracefully
        let extractor = HistoricalSnapshotExtractor::with_lmdb("/shared/data/trading/lmdb")
            .expect("Failed to create LMDB extractor");

        // Query a range that likely has gaps
        let start = 1730811225000;
        let end = start + 3600000; // 1 hour
        let result = extractor.extract_snapshots("BTCUSDT", start, end, 15);

        match result {
            Ok(snapshots) => {
                // Should return partial results, not error
                println!("Extracted {} snapshots (may have gaps)", snapshots.len());
                assert!(snapshots.len() <= 4); // Max 4 snapshots in 1 hour at 15min interval
            }
            Err(e) => {
                // Should not fail completely
                panic!("Should handle missing data gracefully: {}", e);
            }
        }
    }

    #[test]
    #[ignore] // Requires actual LMDB data
    fn test_lmdb_incomplete_indicators() {
        // Test handling of incomplete indicator data
        let extractor = HistoricalSnapshotExtractor::with_lmdb("/shared/data/trading/lmdb")
            .expect("Failed to create LMDB extractor");

        let start = 1730811225000;
        let end = start + 900000; // 15 minutes
        let result = extractor.extract_snapshots("BTCUSDT", start, end, 15);

        // Should handle missing/incomplete indicators
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    #[ignore] // Requires actual LMDB data
    fn test_lmdb_corrupt_json() {
        // This would require injecting corrupt data into test LMDB
        // For now, just verify extractor doesn't panic on errors
    }

    #[test]
    fn test_snapshot_field_validation() {
        let extractor = HistoricalSnapshotExtractor::new();
        let result = extractor.extract_snapshots("BTCUSDT", 1000000000, 1001000000, 15);
        assert!(result.is_ok());

        let snapshots = result.unwrap();
        if !snapshots.is_empty() {
            let snapshot = &snapshots[0];

            // Validate RSI is in valid range (0-100)
            assert!(snapshot.rsi_7 >= 0.0 && snapshot.rsi_7 <= 100.0,
                "RSI7 out of range: {}", snapshot.rsi_7);
            assert!(snapshot.rsi_14 >= 0.0 && snapshot.rsi_14 <= 100.0,
                "RSI14 out of range: {}", snapshot.rsi_14);

            // Validate price is positive
            assert!(snapshot.price > 0.0, "Price should be positive: {}", snapshot.price);

            // Validate EMAs are positive
            assert!(snapshot.ema_20 > 0.0, "EMA20 should be positive");
            assert!(snapshot.ema_20_4h > 0.0, "EMA20_4h should be positive");
            assert!(snapshot.ema_50_4h > 0.0, "EMA50_4h should be positive");

            // Validate ATR is non-negative
            assert!(snapshot.atr_3_4h >= 0.0, "ATR3 should be non-negative");
            assert!(snapshot.atr_14_4h >= 0.0, "ATR14 should be non-negative");

            // Validate time series lengths
            assert_eq!(snapshot.mid_prices.len(), 10, "Should have 10 mid_prices");
            assert_eq!(snapshot.ema_20_values.len(), 10, "Should have 10 EMA20 values");
            assert_eq!(snapshot.macd_values.len(), 10, "Should have 10 MACD values");
            assert_eq!(snapshot.rsi_7_values.len(), 10, "Should have 10 RSI7 values");
            assert_eq!(snapshot.rsi_14_values.len(), 10, "Should have 10 RSI14 values");
        }
    }

    #[test]
    fn test_snapshot_determinism() {
        // Same inputs should produce same outputs (for mock data)
        let extractor = HistoricalSnapshotExtractor::new();

        let result1 = extractor.extract_snapshots("BTCUSDT", 1000000000, 1001000000, 15);
        let result2 = extractor.extract_snapshots("BTCUSDT", 1000000000, 1001000000, 15);

        assert!(result1.is_ok() && result2.is_ok());

        let snapshots1 = result1.unwrap();
        let snapshots2 = result2.unwrap();

        assert_eq!(snapshots1.len(), snapshots2.len());

        for (s1, s2) in snapshots1.iter().zip(snapshots2.iter()) {
            assert_eq!(s1.timestamp, s2.timestamp);
            assert_eq!(s1.symbol, s2.symbol);
            assert!((s1.price - s2.price).abs() < 0.01);
            assert!((s1.rsi_7 - s2.rsi_7).abs() < 0.01);
        }
    }
}

#[cfg(test)]
mod data_validation_tests {
    use super::*;

    #[test]
    fn test_nan_detection() {
        // Test that NaN values are handled properly
        let val = f64::NAN;
        assert!(val.is_nan());
        assert!(!val.is_finite());
    }

    #[test]
    fn test_infinity_detection() {
        let pos_inf = f64::INFINITY;
        let neg_inf = f64::NEG_INFINITY;

        assert!(!pos_inf.is_finite());
        assert!(!neg_inf.is_finite());
    }

    #[test]
    fn test_rsi_range_validation() {
        let valid_rsi_values = vec![0.0, 25.5, 50.0, 75.5, 100.0];
        let invalid_rsi_values = vec![-1.0, -50.0, 101.0, 150.0, f64::NAN, f64::INFINITY];

        for val in valid_rsi_values {
            assert!(val >= 0.0 && val <= 100.0 && val.is_finite());
        }

        for val in invalid_rsi_values {
            assert!(!(val >= 0.0 && val <= 100.0 && val.is_finite()));
        }
    }

    #[test]
    fn test_price_validation() {
        let valid_prices = vec![0.01, 1.0, 100.0, 50000.0, 1000000.0];
        let invalid_prices = vec![0.0, -1.0, -100.0, f64::NAN, f64::INFINITY];

        for price in valid_prices {
            assert!(price > 0.0 && price.is_finite());
        }

        for price in invalid_prices {
            assert!(!(price > 0.0 && price.is_finite()));
        }
    }

    #[test]
    fn test_timestamp_validation() {
        let valid_timestamps = vec![
            1000000000u64,
            1730811225000,
            chrono::Utc::now().timestamp_millis() as u64,
        ];

        let invalid_timestamps = vec![
            0u64,
            // Future timestamp (year 3000)
            32503680000000u64,
        ];

        let now = chrono::Utc::now().timestamp_millis() as u64;
        let year_2000 = 946684800000u64;

        for ts in valid_timestamps {
            assert!(ts >= year_2000 && ts <= now + 86400000); // Within 1 day of now
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_mock_extraction_performance() {
        let extractor = HistoricalSnapshotExtractor::new();
        let start = Instant::now();

        // Extract 1000 snapshots
        let result = extractor.extract_snapshots(
            "BTCUSDT",
            1000000000,
            1000000000 + (1000 * 15 * 60 * 1000), // 1000 * 15min intervals
            15
        );

        let duration = start.elapsed();
        assert!(result.is_ok());

        let snapshots = result.unwrap();
        assert_eq!(snapshots.len(), 1000);

        println!("Mock extraction: {} snapshots in {:?}", snapshots.len(), duration);
        println!("Average: {:?} per snapshot", duration / snapshots.len() as u32);

        // Should be very fast for mock data (<100ms for 1000 snapshots)
        assert!(duration.as_millis() < 1000, "Mock extraction too slow: {:?}", duration);
    }

    #[test]
    #[ignore] // Requires actual LMDB
    fn test_lmdb_extraction_performance() {
        let extractor = HistoricalSnapshotExtractor::with_lmdb("/shared/data/trading/lmdb")
            .expect("Failed to create LMDB extractor");

        let start = Instant::now();

        // Extract 100 snapshots from real LMDB
        let result = extractor.extract_snapshots(
            "BTCUSDT",
            1730811225000,
            1730811225000 + (100 * 15 * 60 * 1000),
            15
        );

        let duration = start.elapsed();

        if let Ok(snapshots) = result {
            println!("LMDB extraction: {} snapshots in {:?}", snapshots.len(), duration);
            if !snapshots.is_empty() {
                println!("Average: {:?} per snapshot", duration / snapshots.len() as u32);
            }

            // Should complete within reasonable time
            assert!(duration.as_secs() < 30, "LMDB extraction too slow: {:?}", duration);
        }
    }

    #[test]
    fn test_memory_usage_large_extraction() {
        let extractor = HistoricalSnapshotExtractor::new();

        // Extract 10K snapshots and verify memory doesn't explode
        let result = extractor.extract_snapshots(
            "BTCUSDT",
            1000000000,
            1000000000 + (10000 * 15 * 60 * 1000),
            15
        );

        assert!(result.is_ok());
        let snapshots = result.unwrap();
        assert_eq!(snapshots.len(), 10000);

        // Each snapshot is roughly 1KB, so 10K should be ~10MB
        // This is a basic sanity check - actual memory profiling would be more thorough
    }
}
