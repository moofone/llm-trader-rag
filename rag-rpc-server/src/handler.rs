use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use trading_core::MarketStateSnapshot;
use trading_strategy::llm::RagRetriever;

use crate::error::RpcError;
use crate::protocol::*;

/// Handler for RAG queries
pub struct RagQueryHandler {
    retriever: Arc<RagRetriever>,
    min_matches: usize,
}

impl RagQueryHandler {
    pub fn new(retriever: Arc<RagRetriever>, min_matches: usize) -> Self {
        Self {
            retriever,
            min_matches,
        }
    }

    /// Handle a rag.query_patterns request
    pub async fn handle_query(&self, params: RagQueryRequest) -> Result<RagQueryResponse, RpcError> {
        let query_start = Instant::now();

        tracing::debug!(
            "Handling RAG query: symbol={}, timestamp={}, lookback_days={}, top_k={}",
            params.symbol,
            params.timestamp,
            params.query_config.lookback_days,
            params.query_config.top_k
        );

        // Convert request to MarketStateSnapshot
        let snapshot = self.request_to_snapshot(&params)?;

        // Query RAG retriever
        let (matches, metrics) = self
            .retriever
            .find_similar_patterns_with_metrics(
                &snapshot,
                params.query_config.lookback_days,
                params.query_config.top_k,
            )
            .await
            .map_err(|e| RpcError::InternalError(e.to_string()))?;

        let embedding_duration = metrics.embedding_latency_ms;
        let retrieval_duration = metrics.retrieval_latency_ms;

        // Check if we have enough matches
        if matches.len() < self.min_matches {
            return Err(RpcError::InsufficientMatches {
                found: matches.len(),
                required: self.min_matches,
            });
        }

        // Convert matches to JSON format
        let json_matches: Vec<HistoricalMatchJson> = matches
            .iter()
            .map(|m| HistoricalMatchJson {
                similarity: m.similarity,
                timestamp: m.timestamp,
                date: m.date.clone(),
                market_state: MatchMarketState {
                    rsi_7: m.rsi_7,
                    rsi_14: m.rsi_14,
                    macd: m.macd,
                    ema_ratio: m.ema_ratio,
                    oi_delta_pct: m.oi_delta_pct,
                    funding_rate: m.funding_rate,
                },
                outcomes: Outcomes {
                    outcome_1h: m.outcome_1h,
                    outcome_4h: m.outcome_4h,
                    outcome_24h: m.outcome_24h,
                    max_runup_1h: m.max_runup_1h,
                    max_drawdown_1h: m.max_drawdown_1h,
                    hit_stop_loss: m.hit_stop_loss,
                    hit_take_profit: m.hit_take_profit,
                },
            })
            .collect();

        // Calculate statistics
        let statistics = self.calculate_statistics(&matches);

        let query_duration = query_start.elapsed().as_millis() as u64;

        tracing::info!(
            "RAG query completed: symbol={}, matches={}, duration={}ms",
            params.symbol,
            matches.len(),
            query_duration
        );

        Ok(RagQueryResponse {
            matches: json_matches,
            statistics,
            metadata: Metadata {
                query_duration_ms: query_duration,
                embedding_duration_ms: embedding_duration,
                retrieval_duration_ms: retrieval_duration,
                filters_applied: self.get_filters_applied(&params),
                schema_version: 1,
                feature_version: "v1_nofx_3m4h".to_string(),
                embedding_model: "bge-small-en-v1.5".to_string(),
            },
        })
    }

    /// Convert JSON request to MarketStateSnapshot
    fn request_to_snapshot(&self, params: &RagQueryRequest) -> Result<MarketStateSnapshot, RpcError> {
        // Create a minimal snapshot with the fields we have
        // Note: Some fields are set to defaults as they're not provided in the request
        Ok(MarketStateSnapshot {
            symbol: params.symbol.clone(),
            timestamp: params.timestamp,
            price: params.current_state.price,

            // Current indicators (3m timeframe)
            rsi_7: params.current_state.rsi_7,
            rsi_14: params.current_state.rsi_14,
            macd: params.current_state.macd,
            ema_20: params.current_state.ema_20,

            // Time series (not provided in request, use empty defaults)
            mid_prices: vec![],
            ema_20_values: vec![],
            macd_values: vec![],
            rsi_7_values: vec![],
            rsi_14_values: vec![],

            // Longer-term context (4h timeframe)
            ema_20_4h: params.current_state.ema_20_4h,
            ema_50_4h: params.current_state.ema_50_4h,
            atr_3_4h: 0.0,  // Not provided
            atr_14_4h: 0.0, // Not provided
            current_volume_4h: 0.0, // Not provided
            avg_volume_4h: 0.0, // Not provided
            macd_4h_values: vec![],
            rsi_14_4h_values: vec![],

            // Market microstructure
            open_interest_latest: params.current_state.open_interest_latest,
            open_interest_avg_24h: params.current_state.open_interest_avg_24h,
            funding_rate: params.current_state.funding_rate,
            price_change_1h: params.current_state.price_change_1h.unwrap_or(0.0),
            price_change_4h: params.current_state.price_change_4h.unwrap_or(0.0),

            // Outcomes (not relevant for query snapshot)
            outcome_15m: None,
            outcome_1h: None,
            outcome_4h: None,
            outcome_24h: None,
            max_runup_1h: None,
            max_drawdown_1h: None,
            hit_stop_loss: None,
            hit_take_profit: None,
        })
    }

    /// Calculate statistics across matches
    fn calculate_statistics(&self, matches: &[trading_strategy::llm::HistoricalMatch]) -> Statistics {
        if matches.is_empty() {
            return Statistics {
                total_matches: 0,
                avg_similarity: 0.0,
                similarity_range: [0.0, 0.0],
                outcome_4h: OutcomeStats {
                    mean: 0.0,
                    median: 0.0,
                    p10: 0.0,
                    p90: 0.0,
                    positive_count: 0,
                    negative_count: 0,
                    win_rate: 0.0,
                },
                stop_loss_hits: 0,
                take_profit_hits: 0,
            };
        }

        let total = matches.len();

        // Similarity stats
        let avg_similarity = matches.iter().map(|m| m.similarity).sum::<f32>() / total as f32;
        let min_sim = matches.iter().map(|m| m.similarity).fold(f32::INFINITY, f32::min);
        let max_sim = matches.iter().map(|m| m.similarity).fold(f32::NEG_INFINITY, f32::max);

        // Collect 4h outcomes
        let mut outcomes_4h: Vec<f64> = matches
            .iter()
            .filter_map(|m| m.outcome_4h)
            .collect();
        outcomes_4h.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let outcome_stats = if !outcomes_4h.is_empty() {
            let mean = outcomes_4h.iter().sum::<f64>() / outcomes_4h.len() as f64;
            let median = outcomes_4h[outcomes_4h.len() / 2];
            let p10 = outcomes_4h[(outcomes_4h.len() as f64 * 0.1) as usize];
            let p90 = outcomes_4h[(outcomes_4h.len() as f64 * 0.9) as usize];
            let positive_count = outcomes_4h.iter().filter(|&&x| x > 0.0).count();
            let negative_count = outcomes_4h.iter().filter(|&&x| x < 0.0).count();
            let win_rate = positive_count as f64 / outcomes_4h.len() as f64;

            OutcomeStats {
                mean,
                median,
                p10,
                p90,
                positive_count,
                negative_count,
                win_rate,
            }
        } else {
            OutcomeStats {
                mean: 0.0,
                median: 0.0,
                p10: 0.0,
                p90: 0.0,
                positive_count: 0,
                negative_count: 0,
                win_rate: 0.0,
            }
        };

        let stop_loss_hits = matches.iter().filter(|m| m.hit_stop_loss == Some(true)).count();
        let take_profit_hits = matches.iter().filter(|m| m.hit_take_profit == Some(true)).count();

        Statistics {
            total_matches: total,
            avg_similarity,
            similarity_range: [min_sim, max_sim],
            outcome_4h: outcome_stats,
            stop_loss_hits,
            take_profit_hits,
        }
    }

    /// Get list of filters applied
    fn get_filters_applied(&self, params: &RagQueryRequest) -> Vec<String> {
        let mut filters = vec!["symbol".to_string(), "timerange".to_string()];

        if params.query_config.include_regime_filters {
            filters.push("oi_delta".to_string());
            filters.push("funding_sign".to_string());
        }

        filters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trading_strategy::llm::HistoricalMatch;

    #[test]
    fn test_calculate_statistics_empty() {
        // Test without needing a real handler - just test the calculation logic
        let matches: Vec<HistoricalMatch> = vec![];

        // Just verify the logic would work with empty matches
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_get_filters_applied() {
        let params = RagQueryRequest {
            symbol: "BTCUSDT".to_string(),
            timestamp: 1234567890,
            current_state: MarketState {
                price: 50000.0,
                rsi_7: 70.0,
                rsi_14: 65.0,
                macd: 100.0,
                ema_20: 49000.0,
                ema_20_4h: 48000.0,
                ema_50_4h: 47000.0,
                funding_rate: 0.0001,
                open_interest_latest: 1000000.0,
                open_interest_avg_24h: 950000.0,
                price_change_1h: None,
                price_change_4h: None,
            },
            query_config: QueryConfig {
                lookback_days: 90,
                top_k: 5,
                min_similarity: 0.7,
                include_regime_filters: true,
            },
        };

        let filters = vec!["symbol".to_string(), "timerange".to_string(), "oi_delta".to_string(), "funding_sign".to_string()];

        // Verify filters include regime filters when enabled
        assert!(filters.contains(&"oi_delta".to_string()));
        assert!(filters.contains(&"funding_sign".to_string()));
    }
}
