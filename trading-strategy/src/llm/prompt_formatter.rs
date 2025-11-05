use super::HistoricalMatch;
use trading_core::MarketStateSnapshot;

/// Formatter for LLM prompts with or without RAG context
pub struct LlmPromptFormatter;

impl LlmPromptFormatter {
    /// Format a baseline prompt without historical context
    ///
    /// Used when insufficient historical matches are found or RAG is disabled
    pub fn format_baseline(symbol: &str, current_snapshot: &MarketStateSnapshot) -> String {
        let mut prompt = String::new();

        prompt.push_str(&format!("═══ {} TRADING ANALYSIS ═══\n\n", symbol));

        // Current market state
        prompt.push_str("CURRENT MARKET STATE:\n");
        prompt.push_str(&format!(
            "  Price: ${:.2}\n",
            current_snapshot.price
        ));
        prompt.push_str(&format!(
            "  RSI(7): {:.1} | RSI(14): {:.1}\n",
            current_snapshot.rsi_7, current_snapshot.rsi_14
        ));
        prompt.push_str(&format!(
            "  MACD: {:.2}\n",
            current_snapshot.macd
        ));
        prompt.push_str(&format!(
            "  EMA(20): ${:.2}\n",
            current_snapshot.ema_20
        ));
        prompt.push_str(&format!(
            "  EMA Ratio (20/50, 4h): {:.4}\n",
            current_snapshot.ema_ratio_20_50()
        ));
        prompt.push_str(&format!(
            "  Open Interest: {:.1}% vs 24h avg\n",
            current_snapshot.oi_delta_pct()
        ));
        prompt.push_str(&format!(
            "  Funding Rate: {:.6}\n",
            current_snapshot.funding_rate
        ));
        prompt.push_str(&format!(
            "  Price Change 1h: {:+.2}% | 4h: {:+.2}%\n",
            current_snapshot.price_change_1h, current_snapshot.price_change_4h
        ));

        prompt.push_str("\n");
        prompt.push_str("⚠️  NO HISTORICAL PATTERN CONTEXT AVAILABLE\n\n");
        prompt.push_str("DECISION REQUIRED:\n");
        prompt.push_str("Based on current indicators only, should the strategy:\n");
        prompt.push_str("  A) LONG - Enter long position\n");
        prompt.push_str("  B) SHORT - Enter short position\n");
        prompt.push_str("  C) HOLD - No position/stay flat\n\n");
        prompt.push_str("Provide your decision with 2-3 sentence reasoning.\n");

        prompt
    }

    /// Format a prompt enriched with historical pattern analysis
    ///
    /// Includes similar patterns, their outcomes, and summary statistics
    pub fn format_with_historical_patterns(
        symbol: &str,
        current_snapshot: &MarketStateSnapshot,
        historical_matches: Vec<HistoricalMatch>,
    ) -> String {
        let mut prompt = String::new();

        prompt.push_str(&format!("═══ {} TRADING ANALYSIS WITH HISTORICAL CONTEXT ═══\n\n", symbol));

        // Current market state
        prompt.push_str("CURRENT MARKET STATE:\n");
        prompt.push_str(&format!(
            "  Price: ${:.2}\n",
            current_snapshot.price
        ));
        prompt.push_str(&format!(
            "  RSI(7): {:.1} | RSI(14): {:.1}\n",
            current_snapshot.rsi_7, current_snapshot.rsi_14
        ));
        prompt.push_str(&format!(
            "  MACD: {:.2}\n",
            current_snapshot.macd
        ));
        prompt.push_str(&format!(
            "  EMA Ratio (20/50): {:.4}\n",
            current_snapshot.ema_ratio_20_50()
        ));
        prompt.push_str(&format!(
            "  OI Delta: {:+.1}% | Funding: {:.6}\n",
            current_snapshot.oi_delta_pct(),
            current_snapshot.funding_rate
        ));
        prompt.push_str(&format!(
            "  Price Change 1h: {:+.2}% | 4h: {:+.2}%\n",
            current_snapshot.price_change_1h, current_snapshot.price_change_4h
        ));

        // Historical pattern analysis
        if !historical_matches.is_empty() {
            prompt.push_str("\n");
            prompt.push_str("═══════════════════════════════════════════════════════════\n");
            prompt.push_str("📊 HISTORICAL PATTERN ANALYSIS\n");
            prompt.push_str("What Happened When Market Looked Like This\n");
            prompt.push_str("═══════════════════════════════════════════════════════════\n\n");

            prompt.push_str(&format!(
                "Found {} similar market conditions from recent history:\n\n",
                historical_matches.len()
            ));

            // Individual matches
            for (i, m) in historical_matches.iter().enumerate() {
                prompt.push_str(&format!(
                    "{}. {} (Similarity: {:.1}%)\n",
                    i + 1,
                    m.date,
                    m.similarity * 100.0
                ));

                prompt.push_str(&format!(
                    "   State: RSI7={:.1}, MACD={:.1}, EMA_Ratio={:.3}, OI{:+.1}%, Fund={:.4}\n",
                    m.rsi_7, m.macd, m.ema_ratio, m.oi_delta_pct, m.funding_rate
                ));

                // Outcomes - the valuable part
                if let Some(outcome_4h) = m.outcome_4h {
                    prompt.push_str(&format!("   → 4h Result: {:+.2}%", outcome_4h));

                    if let (Some(runup), Some(drawdown)) = (m.max_runup_1h, m.max_drawdown_1h) {
                        prompt.push_str(&format!(
                            " (peak: {:+.1}%, trough: {:+.1}%)",
                            runup, drawdown
                        ));
                    }

                    if m.hit_stop_loss == Some(true) {
                        prompt.push_str(" ❌ HIT STOP");
                    } else if m.hit_take_profit == Some(true) {
                        prompt.push_str(" ✅ HIT TARGET");
                    }

                    prompt.push_str("\n");
                }

                prompt.push_str("\n");
            }

            // Summary statistics
            let stats = OutcomeStatistics::calculate(&historical_matches);

            prompt.push_str("OUTCOME SUMMARY (4h horizon):\n");
            prompt.push_str(&format!("  Average: {:+.2}%\n", stats.avg_outcome_4h));
            prompt.push_str(&format!(
                "  Median: {:+.2}% | P10: {:+.2}% | P90: {:+.2}%\n",
                stats.median_outcome_4h, stats.p10_outcome_4h, stats.p90_outcome_4h
            ));
            prompt.push_str(&format!(
                "  Positive: {}/{} ({:.0}%) | Negative: {}/{} ({:.0}%)\n",
                stats.positive_count,
                stats.total_count,
                stats.positive_pct,
                stats.negative_count,
                stats.total_count,
                stats.negative_pct
            ));
            prompt.push_str(&format!(
                "  Stop Loss Hit: {} | Take Profit Hit: {}\n",
                stats.stop_loss_hits, stats.take_profit_hits
            ));
            prompt.push_str(&format!(
                "  Similarity Range: {:.0}%-{:.0}%\n",
                stats.min_similarity * 100.0,
                stats.max_similarity * 100.0
            ));
        } else {
            prompt.push_str("\n");
            prompt.push_str("[No similar historical patterns found - using current data only]\n");
        }

        // Decision prompt
        prompt.push_str("\n");
        prompt.push_str("═══════════════════════════════════════════════════════════\n");
        prompt.push_str("DECISION REQUIRED:\n\n");
        prompt.push_str("Based on the CURRENT STATE and HISTORICAL OUTCOMES, choose:\n");
        prompt.push_str("  A) LONG - Enter long position\n");
        prompt.push_str("  B) SHORT - Enter short position\n");
        prompt.push_str("  C) HOLD - No position/stay flat\n\n");
        prompt.push_str("Consider that historical outcomes provide empirical evidence about\n");
        prompt.push_str("what typically happens in similar market conditions.\n");
        prompt.push_str("Weight this evidence appropriately in your decision.\n\n");
        prompt.push_str("Provide your decision with 2-3 sentence reasoning.\n");

        prompt
    }
}

/// Statistics calculated from historical outcomes
struct OutcomeStatistics {
    avg_outcome_4h: f64,
    median_outcome_4h: f64,
    p10_outcome_4h: f64,
    p90_outcome_4h: f64,
    positive_count: usize,
    negative_count: usize,
    total_count: usize,
    positive_pct: f64,
    negative_pct: f64,
    stop_loss_hits: usize,
    take_profit_hits: usize,
    min_similarity: f32,
    max_similarity: f32,
}

impl OutcomeStatistics {
    fn calculate(matches: &[HistoricalMatch]) -> Self {
        let outcomes_4h: Vec<f64> = matches
            .iter()
            .filter_map(|m| m.outcome_4h)
            .collect();

        let total_count = outcomes_4h.len();

        if outcomes_4h.is_empty() {
            return Self::default();
        }

        // Sort for percentile calculations
        let mut sorted = outcomes_4h.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Percentile function
        let percentile = |p: f64| -> f64 {
            let len = sorted.len();
            let idx = ((len as f64 - 1.0) * p).round() as usize;
            sorted[idx.min(len - 1)]
        };

        let avg = sorted.iter().sum::<f64>() / total_count as f64;
        let median = percentile(0.5);
        let p10 = percentile(0.10);
        let p90 = percentile(0.90);

        let positive_count = outcomes_4h.iter().filter(|&&x| x > 0.0).count();
        let negative_count = outcomes_4h.iter().filter(|&&x| x < 0.0).count();

        let positive_pct = (positive_count as f64 / total_count as f64) * 100.0;
        let negative_pct = (negative_count as f64 / total_count as f64) * 100.0;

        let stop_loss_hits = matches
            .iter()
            .filter(|m| m.hit_stop_loss == Some(true))
            .count();

        let take_profit_hits = matches
            .iter()
            .filter(|m| m.hit_take_profit == Some(true))
            .count();

        let min_similarity = matches
            .iter()
            .map(|m| m.similarity)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let max_similarity = matches
            .iter()
            .map(|m| m.similarity)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        Self {
            avg_outcome_4h: avg,
            median_outcome_4h: median,
            p10_outcome_4h: p10,
            p90_outcome_4h: p90,
            positive_count,
            negative_count,
            total_count,
            positive_pct,
            negative_pct,
            stop_loss_hits,
            take_profit_hits,
            min_similarity,
            max_similarity,
        }
    }
}

impl Default for OutcomeStatistics {
    fn default() -> Self {
        Self {
            avg_outcome_4h: 0.0,
            median_outcome_4h: 0.0,
            p10_outcome_4h: 0.0,
            p90_outcome_4h: 0.0,
            positive_count: 0,
            negative_count: 0,
            total_count: 0,
            positive_pct: 0.0,
            negative_pct: 0.0,
            stop_loss_hits: 0,
            take_profit_hits: 0,
            min_similarity: 0.0,
            max_similarity: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_prompt_format() {
        let snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);
        let prompt = LlmPromptFormatter::format_baseline("BTCUSDT", &snapshot);

        assert!(prompt.contains("BTCUSDT"));
        assert!(prompt.contains("CURRENT MARKET STATE"));
        assert!(prompt.contains("NO HISTORICAL PATTERN CONTEXT"));
        assert!(prompt.contains("DECISION REQUIRED"));
    }

    #[test]
    fn test_rag_prompt_with_matches() {
        let snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);

        let matches = vec![
            HistoricalMatch {
                similarity: 0.85,
                timestamp: 1000000,
                date: "2025-10-01T00:00:00Z".to_string(),
                rsi_7: 75.0,
                rsi_14: 72.0,
                macd: 50.0,
                ema_ratio: 1.01,
                oi_delta_pct: 5.0,
                funding_rate: 0.0001,
                outcome_1h: Some(2.0),
                outcome_4h: Some(-1.5),
                outcome_24h: Some(3.0),
                max_runup_1h: Some(2.5),
                max_drawdown_1h: Some(-0.5),
                hit_stop_loss: Some(false),
                hit_take_profit: Some(true),
            },
            HistoricalMatch {
                similarity: 0.80,
                timestamp: 2000000,
                date: "2025-10-02T00:00:00Z".to_string(),
                rsi_7: 78.0,
                rsi_14: 74.0,
                macd: 55.0,
                ema_ratio: 1.02,
                oi_delta_pct: 6.0,
                funding_rate: 0.0002,
                outcome_1h: Some(1.5),
                outcome_4h: Some(2.0),
                outcome_24h: Some(4.0),
                max_runup_1h: Some(3.0),
                max_drawdown_1h: Some(-0.2),
                hit_stop_loss: Some(false),
                hit_take_profit: Some(true),
            },
        ];

        let prompt = LlmPromptFormatter::format_with_historical_patterns(
            "BTCUSDT",
            &snapshot,
            matches,
        );

        assert!(prompt.contains("BTCUSDT"));
        assert!(prompt.contains("HISTORICAL PATTERN ANALYSIS"));
        assert!(prompt.contains("Found 2 similar market conditions"));
        assert!(prompt.contains("OUTCOME SUMMARY"));
        assert!(prompt.contains("Average:"));
        assert!(prompt.contains("Median:"));
    }

    #[test]
    fn test_outcome_statistics() {
        let matches = vec![
            HistoricalMatch {
                similarity: 0.85,
                timestamp: 1000000,
                date: "2025-10-01".to_string(),
                rsi_7: 75.0,
                rsi_14: 72.0,
                macd: 50.0,
                ema_ratio: 1.01,
                oi_delta_pct: 5.0,
                funding_rate: 0.0001,
                outcome_1h: Some(2.0),
                outcome_4h: Some(-2.0),
                outcome_24h: Some(3.0),
                max_runup_1h: Some(2.5),
                max_drawdown_1h: Some(-0.5),
                hit_stop_loss: Some(true),
                hit_take_profit: Some(false),
            },
            HistoricalMatch {
                similarity: 0.90,
                timestamp: 2000000,
                date: "2025-10-02".to_string(),
                rsi_7: 78.0,
                rsi_14: 74.0,
                macd: 55.0,
                ema_ratio: 1.02,
                oi_delta_pct: 6.0,
                funding_rate: 0.0002,
                outcome_1h: Some(1.5),
                outcome_4h: Some(3.0),
                outcome_24h: Some(4.0),
                max_runup_1h: Some(3.0),
                max_drawdown_1h: Some(-0.2),
                hit_stop_loss: Some(false),
                hit_take_profit: Some(true),
            },
            HistoricalMatch {
                similarity: 0.75,
                timestamp: 3000000,
                date: "2025-10-03".to_string(),
                rsi_7: 76.0,
                rsi_14: 73.0,
                macd: 52.0,
                ema_ratio: 1.015,
                oi_delta_pct: 5.5,
                funding_rate: 0.00015,
                outcome_1h: Some(1.0),
                outcome_4h: Some(1.0),
                outcome_24h: Some(2.0),
                max_runup_1h: Some(2.0),
                max_drawdown_1h: Some(-0.3),
                hit_stop_loss: Some(false),
                hit_take_profit: Some(false),
            },
        ];

        let stats = OutcomeStatistics::calculate(&matches);

        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.positive_count, 2);
        assert_eq!(stats.negative_count, 1);
        assert!((stats.avg_outcome_4h - 0.666).abs() < 0.01);
        assert_eq!(stats.stop_loss_hits, 1);
        assert_eq!(stats.take_profit_hits, 1);
        assert_eq!(stats.min_similarity, 0.75);
        assert_eq!(stats.max_similarity, 0.90);
    }
}
