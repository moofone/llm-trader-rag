use trading_core::MarketStateSnapshot;
use trading_strategy::{HistoricalMatch, LlmPromptFormatter};

/// End-to-end integration test for Phase 2: Live Pattern Retrieval
///
/// This test verifies the complete flow:
/// 1. Create a current market snapshot
/// 2. Create mock historical matches (simulating RAG retrieval)
/// 3. Format baseline prompt
/// 4. Format RAG-enhanced prompt with historical patterns
/// 5. Verify prompt structure and content
#[test]
fn test_phase2_end_to_end_prompt_generation() {
    // Step 1: Create current market snapshot using the builder
    let mut current_snapshot = MarketStateSnapshot::new(
        "BTCUSDT".to_string(),
        1700000000000,
        43250.0,
    );

    // Set key indicators
    current_snapshot.rsi_7 = 78.5;
    current_snapshot.rsi_14 = 72.3;
    current_snapshot.macd = 125.5;
    current_snapshot.ema_20 = 43100.0;
    current_snapshot.funding_rate = 0.0005;
    current_snapshot.price_change_1h = 1.2;
    current_snapshot.price_change_4h = 2.8;
    current_snapshot.open_interest_latest = 1000000.0;
    current_snapshot.open_interest_avg_24h = 950000.0;

    // Step 2: Create mock historical matches (simulating RAG retrieval results)
    let historical_matches = vec![
        // Match 1: Strong uptrend that succeeded
        HistoricalMatch {
            similarity: 0.92,
            timestamp: 1699000000000,
            date: "2023-11-03T12:00:00Z".to_string(),
            rsi_7: 79.2,
            rsi_14: 73.1,
            macd: 128.0,
            ema_ratio: 1.008,
            oi_delta_pct: 9.2,
            funding_rate: 0.0006,
            outcome_1h: Some(1.8),
            outcome_4h: Some(3.5),
            outcome_24h: Some(5.2),
            max_runup_1h: Some(2.3),
            max_drawdown_1h: Some(-0.4),
            hit_stop_loss: Some(false),
            hit_take_profit: Some(true),
        },
        // Match 2: Similar setup that failed
        HistoricalMatch {
            similarity: 0.88,
            timestamp: 1698000000000,
            date: "2023-10-23T08:00:00Z".to_string(),
            rsi_7: 77.5,
            rsi_14: 71.8,
            macd: 122.0,
            ema_ratio: 1.007,
            oi_delta_pct: 7.8,
            funding_rate: 0.0005,
            outcome_1h: Some(-2.1),
            outcome_4h: Some(-4.2),
            outcome_24h: Some(-3.8),
            max_runup_1h: Some(0.8),
            max_drawdown_1h: Some(-2.5),
            hit_stop_loss: Some(true),
            hit_take_profit: Some(false),
        },
        // Match 3: Consolidation then breakout
        HistoricalMatch {
            similarity: 0.85,
            timestamp: 1697000000000,
            date: "2023-10-11T16:00:00Z".to_string(),
            rsi_7: 76.2,
            rsi_14: 70.5,
            macd: 118.5,
            ema_ratio: 1.006,
            oi_delta_pct: 8.9,
            funding_rate: 0.0004,
            outcome_1h: Some(0.5),
            outcome_4h: Some(2.8),
            outcome_24h: Some(4.1),
            max_runup_1h: Some(1.2),
            max_drawdown_1h: Some(-0.3),
            hit_stop_loss: Some(false),
            hit_take_profit: Some(true),
        },
        // Match 4: Quick reversal
        HistoricalMatch {
            similarity: 0.82,
            timestamp: 1696000000000,
            date: "2023-09-30T10:00:00Z".to_string(),
            rsi_7: 80.1,
            rsi_14: 74.2,
            macd: 130.2,
            ema_ratio: 1.009,
            oi_delta_pct: 10.1,
            funding_rate: 0.0007,
            outcome_1h: Some(1.2),
            outcome_4h: Some(-1.5),
            outcome_24h: Some(0.3),
            max_runup_1h: Some(1.8),
            max_drawdown_1h: Some(-0.6),
            hit_stop_loss: Some(false),
            hit_take_profit: Some(false),
        },
        // Match 5: Strong continuation
        HistoricalMatch {
            similarity: 0.79,
            timestamp: 1695000000000,
            date: "2023-09-18T14:00:00Z".to_string(),
            rsi_7: 75.8,
            rsi_14: 69.9,
            macd: 115.0,
            ema_ratio: 1.005,
            oi_delta_pct: 7.2,
            funding_rate: 0.0003,
            outcome_1h: Some(2.5),
            outcome_4h: Some(4.8),
            outcome_24h: Some(6.5),
            max_runup_1h: Some(2.9),
            max_drawdown_1h: Some(-0.2),
            hit_stop_loss: Some(false),
            hit_take_profit: Some(true),
        },
    ];

    // Step 3: Test baseline prompt (no RAG context)
    let baseline_prompt = LlmPromptFormatter::format_baseline("BTCUSDT", &current_snapshot);

    println!("\n=== BASELINE PROMPT ===");
    println!("{}", baseline_prompt);

    // Verify baseline prompt structure
    assert!(baseline_prompt.contains("BTCUSDT"));
    assert!(baseline_prompt.contains("$43250.00"));
    assert!(baseline_prompt.contains("RSI(7): 78.5"));
    assert!(baseline_prompt.contains("RSI(14): 72.3"));
    assert!(baseline_prompt.contains("MACD: 125.5"));
    assert!(baseline_prompt.contains("DECISION REQUIRED"));
    assert!(baseline_prompt.contains("LONG"));
    assert!(baseline_prompt.contains("SHORT"));
    assert!(baseline_prompt.contains("HOLD"));

    // Step 4: Test RAG-enhanced prompt
    let rag_prompt = LlmPromptFormatter::format_with_historical_patterns(
        "BTCUSDT",
        &current_snapshot,
        historical_matches,
    );

    println!("\n=== RAG-ENHANCED PROMPT ===");
    println!("{}", rag_prompt);

    // Verify RAG prompt structure
    assert!(rag_prompt.contains("BTCUSDT"));
    assert!(rag_prompt.contains("CURRENT MARKET STATE"));
    assert!(rag_prompt.contains("HISTORICAL PATTERN ANALYSIS"));
    assert!(rag_prompt.contains("2023-11-03")); // First match date
    assert!(rag_prompt.contains("Similarity: 92.0%")); // First match similarity
    assert!(rag_prompt.contains("4h Result:")); // Outcome section
    assert!(rag_prompt.contains("OUTCOME SUMMARY"));
    assert!(rag_prompt.contains("Average:"));
    assert!(rag_prompt.contains("Positive:") || rag_prompt.contains("Negative:"));

    // Verify RAG prompt is longer and more detailed than baseline
    assert!(rag_prompt.len() > baseline_prompt.len());
    assert!(rag_prompt.len() > 2000); // Should be substantial with 5 matches

    // Step 5: Verify statistical calculations are present
    assert!(rag_prompt.contains("P10:")); // Percentiles
    assert!(rag_prompt.contains("P90:"));
    assert!(rag_prompt.contains("Median:"));
    assert!(rag_prompt.contains("Positive:") || rag_prompt.contains("positive")); // Win/loss counts
    assert!(rag_prompt.contains("Negative:") || rag_prompt.contains("negative"));

    println!("\n=== TEST SUMMARY ===");
    println!("✅ Baseline prompt generated ({} chars)", baseline_prompt.len());
    println!("✅ RAG-enhanced prompt generated ({} chars)", rag_prompt.len());
    println!("✅ All prompt components verified");
    println!("✅ Phase 2 end-to-end flow working correctly");
}

/// Test the quality of prompts with edge cases
#[test]
fn test_phase2_edge_cases() {
    let mut current_snapshot = MarketStateSnapshot::new(
        "ETHUSDT".to_string(),
        1700000000000,
        2250.0,
    );

    // Oversold conditions
    current_snapshot.rsi_7 = 30.0;
    current_snapshot.rsi_14 = 35.0;
    current_snapshot.macd = -25.0;
    current_snapshot.funding_rate = -0.0003; // Negative funding
    current_snapshot.price_change_1h = -3.5;
    current_snapshot.price_change_4h = -8.2;

    // Test with minimal matches (below typical threshold)
    let minimal_matches = vec![
        HistoricalMatch {
            similarity: 0.75,
            timestamp: 1699000000000,
            date: "2023-11-03T12:00:00Z".to_string(),
            rsi_7: 32.0,
            rsi_14: 36.0,
            macd: -22.0,
            ema_ratio: 0.985,
            oi_delta_pct: -11.0,
            funding_rate: -0.0002,
            outcome_1h: Some(3.2), // Reversal bounce
            outcome_4h: Some(5.5),
            outcome_24h: Some(2.8),
            max_runup_1h: Some(3.8),
            max_drawdown_1h: Some(-0.5),
            hit_stop_loss: Some(false),
            hit_take_profit: Some(true),
        },
    ];

    let prompt = LlmPromptFormatter::format_with_historical_patterns(
        "ETHUSDT",
        &current_snapshot,
        minimal_matches,
    );

    // Verify it handles edge cases correctly
    assert!(prompt.contains("ETHUSDT"));
    assert!(prompt.contains("30.0")); // Oversold RSI
    assert!(prompt.contains("-25")); // Negative MACD (may be formatted differently)
    assert!(prompt.contains("HISTORICAL PATTERN ANALYSIS")); // Even with 1 match

    println!("\n✅ Edge case handling verified");
}

/// Test with empty historical matches (fallback to baseline)
#[test]
fn test_phase2_no_matches_fallback() {
    let mut current_snapshot = MarketStateSnapshot::new(
        "SOLUSDT".to_string(),
        1700000000000,
        125.50,
    );

    current_snapshot.rsi_7 = 55.0;
    current_snapshot.rsi_14 = 52.0;
    current_snapshot.macd = 5.0;
    current_snapshot.funding_rate = 0.0001;
    current_snapshot.price_change_1h = 0.5;
    current_snapshot.price_change_4h = 1.2;

    // Test with empty matches
    let empty_matches: Vec<HistoricalMatch> = vec![];

    let prompt = LlmPromptFormatter::format_with_historical_patterns(
        "SOLUSDT",
        &current_snapshot,
        empty_matches,
    );

    // When there are no matches, it should still produce a valid prompt
    // (though in practice, RagRetriever would return empty vec and strategy would use baseline)
    assert!(prompt.contains("SOLUSDT"));
    assert!(prompt.contains("$125.50"));

    println!("\n✅ Empty matches handling verified");
}
