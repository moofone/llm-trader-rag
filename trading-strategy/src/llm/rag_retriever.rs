use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use qdrant_client::qdrant::{Condition, Filter, Range};
use std::collections::HashMap;
use std::sync::Arc;
use trading_core::MarketStateSnapshot;
use trading_data_services::{SnapshotFormatter, VectorStore};

/// A historical pattern match with its market state and outcomes
#[derive(Debug, Clone)]
pub struct HistoricalMatch {
    pub similarity: f32, // 0.0 to 1.0 (cosine similarity)
    pub timestamp: u64,
    pub date: String,

    // State at that time
    pub rsi_7: f64,
    pub rsi_14: f64,
    pub macd: f64,
    pub ema_ratio: f64,
    pub oi_delta_pct: f64,
    pub funding_rate: f64,

    // What happened next (THE VALUE)
    pub outcome_1h: Option<f64>,
    pub outcome_4h: Option<f64>,
    pub outcome_24h: Option<f64>,
    pub max_runup_1h: Option<f64>,
    pub max_drawdown_1h: Option<f64>,
    pub hit_stop_loss: Option<bool>,
    pub hit_take_profit: Option<bool>,
}

/// RAG retriever for finding similar historical patterns
pub struct RagRetriever {
    embedding_model: TextEmbedding,
    vector_store: Arc<VectorStore>,
    min_matches: usize,
}

impl RagRetriever {
    /// Create a new RAG retriever
    pub async fn new(vector_store: Arc<VectorStore>, min_matches: usize) -> Result<Self> {
        tracing::info!("Initializing RAG retriever with BGE-small-en-v1.5 model...");

        let embedding_model = TextEmbedding::try_new(InitOptions::new(
            EmbeddingModel::BGESmallENV15,
        ))?;

        tracing::info!("RAG retriever initialized successfully");

        Ok(Self {
            embedding_model,
            vector_store,
            min_matches,
        })
    }

    /// Find similar historical patterns for the current market state
    ///
    /// # Arguments
    /// * `current_snapshot` - Current market state
    /// * `lookback_days` - How many days back to search
    /// * `top_k` - Maximum number of similar patterns to return
    ///
    /// # Returns
    /// Vector of historical matches, empty if fewer than `min_matches` found
    pub async fn find_similar_patterns(
        &self,
        current_snapshot: &MarketStateSnapshot,
        lookback_days: u32,
        top_k: usize,
    ) -> Result<Vec<HistoricalMatch>> {
        tracing::debug!(
            "Searching for similar patterns: symbol={}, lookback_days={}, top_k={}",
            current_snapshot.symbol,
            lookback_days,
            top_k
        );

        // 1. Convert current state to embedding
        let query_text = current_snapshot.to_embedding_text();
        let query_embedding = self
            .embedding_model
            .embed(vec![query_text], None)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Failed to generate embedding"))?;

        tracing::debug!("Generated query embedding (384 dimensions)");

        // 2. Build filter for recency and symbol
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let lookback_ms = lookback_days as u64 * 86400 * 1000;
        let min_timestamp = now_ms.saturating_sub(lookback_ms);

        let mut conditions = vec![
            // Must match symbol
            Condition {
                condition_one_of: Some(
                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        qdrant_client::qdrant::FieldCondition {
                            key: "symbol".to_string(),
                            r#match: Some(qdrant_client::qdrant::Match {
                                match_value: Some(
                                    qdrant_client::qdrant::r#match::MatchValue::Keyword(
                                        current_snapshot.symbol.clone(),
                                    ),
                                ),
                            }),
                            ..Default::default()
                        },
                    ),
                ),
            },
            // Must be within lookback window
            Condition {
                condition_one_of: Some(
                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        qdrant_client::qdrant::FieldCondition {
                            key: "timestamp".to_string(),
                            range: Some(Range {
                                gte: Some(min_timestamp as f64),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ),
                ),
            },
        ];

        // Optional: Filter by OI delta regime (if significant)
        let oi_delta = current_snapshot.oi_delta_pct();
        if oi_delta.abs() > 5.0 {
            let oi_min = oi_delta - 10.0;
            let oi_max = oi_delta + 10.0;
            conditions.push(Condition {
                condition_one_of: Some(
                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        qdrant_client::qdrant::FieldCondition {
                            key: "oi_delta_pct".to_string(),
                            range: Some(Range {
                                gte: Some(oi_min),
                                lte: Some(oi_max),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ),
                ),
            });
            tracing::debug!("Applied OI delta filter: {}% ±10%", oi_delta);
        }

        // Optional: Filter by funding rate sign
        if current_snapshot.funding_rate.abs() > 0.0001 {
            let funding_condition = if current_snapshot.funding_rate > 0.0 {
                Range {
                    gte: Some(0.0),
                    ..Default::default()
                }
            } else {
                Range {
                    lte: Some(0.0),
                    ..Default::default()
                }
            };

            conditions.push(Condition {
                condition_one_of: Some(
                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                        qdrant_client::qdrant::FieldCondition {
                            key: "funding_rate".to_string(),
                            range: Some(funding_condition),
                            ..Default::default()
                        },
                    ),
                ),
            });
            tracing::debug!(
                "Applied funding rate filter: {} sign",
                if current_snapshot.funding_rate > 0.0 {
                    "positive"
                } else {
                    "negative"
                }
            );
        }

        let filter = Filter {
            must: conditions,
            ..Default::default()
        };

        // 3. Search Qdrant
        let scored_points = self
            .vector_store
            .search(
                query_embedding,
                top_k as u64,
                Some(filter),
                Some(0.7), // Only return good matches (>70% similarity)
            )
            .await?;

        tracing::info!(
            "Found {} similar patterns (similarity threshold: 0.7)",
            scored_points.len()
        );

        // 4. Parse results into HistoricalMatch structs
        let mut matches = Vec::new();

        for scored_point in scored_points {
            let payload = scored_point.payload;

            let historical_match = HistoricalMatch {
                similarity: scored_point.score,
                timestamp: Self::get_payload_u64(&payload, "timestamp")?,
                date: Self::get_payload_string(&payload, "date")?,
                rsi_7: Self::get_payload_f64(&payload, "rsi_7")?,
                rsi_14: Self::get_payload_f64(&payload, "rsi_14")?,
                macd: Self::get_payload_f64(&payload, "macd")?,
                ema_ratio: Self::get_payload_f64(&payload, "ema_ratio")?,
                oi_delta_pct: Self::get_payload_f64(&payload, "oi_delta_pct")?,
                funding_rate: Self::get_payload_f64(&payload, "funding_rate")?,
                outcome_1h: Self::get_payload_f64_opt(&payload, "outcome_1h"),
                outcome_4h: Self::get_payload_f64_opt(&payload, "outcome_4h"),
                outcome_24h: Self::get_payload_f64_opt(&payload, "outcome_24h"),
                max_runup_1h: Self::get_payload_f64_opt(&payload, "max_runup_1h"),
                max_drawdown_1h: Self::get_payload_f64_opt(&payload, "max_drawdown_1h"),
                hit_stop_loss: Self::get_payload_bool_opt(&payload, "hit_stop_loss"),
                hit_take_profit: Self::get_payload_bool_opt(&payload, "hit_take_profit"),
            };

            matches.push(historical_match);
        }

        // 5. Enforce minimum match count (fallback to baseline if insufficient)
        if matches.len() < self.min_matches {
            tracing::warn!(
                "Insufficient matches: found {}, need {}. Returning empty (will use baseline prompt)",
                matches.len(),
                self.min_matches
            );
            return Ok(Vec::new());
        }

        tracing::info!(
            "Successfully retrieved {} historical matches (min={}, similarity_range={:.2}-{:.2})",
            matches.len(),
            self.min_matches,
            matches.iter().map(|m| m.similarity).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0),
            matches.iter().map(|m| m.similarity).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0),
        );

        Ok(matches)
    }

    // Helper methods for payload extraction
    fn get_payload_f64(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
        key: &str,
    ) -> Result<f64> {
        payload
            .get(key)
            .and_then(|v| v.kind.as_ref())
            .and_then(|kind| match kind {
                qdrant_client::qdrant::value::Kind::DoubleValue(d) => Some(*d),
                qdrant_client::qdrant::value::Kind::IntegerValue(i) => Some(*i as f64),
                _ => None,
            })
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
    }

    fn get_payload_f64_opt(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
        key: &str,
    ) -> Option<f64> {
        payload.get(key).and_then(|v| v.kind.as_ref()).and_then(
            |kind| match kind {
                qdrant_client::qdrant::value::Kind::DoubleValue(d) => Some(*d),
                qdrant_client::qdrant::value::Kind::IntegerValue(i) => Some(*i as f64),
                _ => None,
            },
        )
    }

    fn get_payload_bool_opt(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
        key: &str,
    ) -> Option<bool> {
        payload.get(key).and_then(|v| v.kind.as_ref()).and_then(
            |kind| match kind {
                qdrant_client::qdrant::value::Kind::BoolValue(b) => Some(*b),
                _ => None,
            },
        )
    }

    fn get_payload_string(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
        key: &str,
    ) -> Result<String> {
        payload
            .get(key)
            .and_then(|v| v.kind.as_ref())
            .and_then(|kind| match kind {
                qdrant_client::qdrant::value::Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
    }

    fn get_payload_u64(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
        key: &str,
    ) -> Result<u64> {
        payload
            .get(key)
            .and_then(|v| v.kind.as_ref())
            .and_then(|kind| match kind {
                qdrant_client::qdrant::value::Kind::IntegerValue(i) => Some(*i as u64),
                qdrant_client::qdrant::value::Kind::DoubleValue(d) => Some(*d as u64),
                _ => None,
            })
            .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_historical_match_creation() {
        let match_result = HistoricalMatch {
            similarity: 0.85,
            timestamp: 1000000,
            date: "2025-01-01T00:00:00Z".to_string(),
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
        };

        assert_eq!(match_result.similarity, 0.85);
        assert_eq!(match_result.outcome_4h, Some(-1.5));
    }

    // Note: Integration tests with real Qdrant will be in a separate test module
}
