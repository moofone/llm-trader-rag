use anyhow::Result;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, Filter, PointStruct, ScoredPoint,
    SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use serde_json;
use trading_core::MarketStateSnapshot;
use tracing;

/// Qdrant vector store for market snapshots
pub struct VectorStore {
    client: Qdrant,
    collection_name: String,
}

impl VectorStore {
    /// Initialize Qdrant client (embedded for dev, cloud for prod)
    pub async fn new(qdrant_url: &str, collection_name: String) -> Result<Self> {
        let client = Qdrant::from_url(qdrant_url).build()?;

        tracing::info!("Connecting to Qdrant at {}", qdrant_url);

        Ok(Self {
            client,
            collection_name,
        })
    }

    /// Create collection if it doesn't exist
    pub async fn create_collection_if_not_exists(&self, dimension: u64) -> Result<()> {
        match self
            .client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name)
                    .vectors_config(VectorParamsBuilder::new(dimension, Distance::Cosine))
            )
            .await
        {
            Ok(_) => {
                tracing::info!("Created Qdrant collection: {}", self.collection_name);
                Ok(())
            }
            Err(e) => {
                // Collection might already exist
                tracing::info!(
                    "Qdrant collection {} already exists or error: {}",
                    self.collection_name,
                    e
                );
                Ok(())
            }
        }
    }

    /// Upload points to Qdrant
    pub async fn upsert_points(&self, points: Vec<PointStruct>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        tracing::info!("Upserting {} points to Qdrant", points.len());

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await?;

        Ok(())
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        query_vector: Vec<f32>,
        limit: u64,
        filter: Option<Filter>,
        score_threshold: Option<f32>,
    ) -> Result<Vec<ScoredPoint>> {
        let mut search_builder = SearchPointsBuilder::new(&self.collection_name, query_vector, limit)
            .with_payload(true);

        if let Some(f) = filter {
            search_builder = search_builder.filter(f);
        }

        if let Some(threshold) = score_threshold {
            search_builder = search_builder.score_threshold(threshold);
        }

        let search_result = self
            .client
            .search_points(search_builder)
            .await?;

        Ok(search_result.result)
    }

    /// Get collection info
    pub async fn collection_info(&self) -> Result<()> {
        match self.client.collection_info(&self.collection_name).await {
            Ok(info) => {
                tracing::info!("Collection info: {:?}", info);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to get collection info: {}", e);
                Err(e.into())
            }
        }
    }
}

/// Helper to create Qdrant points from snapshots
pub fn snapshot_to_point(
    snapshot: &MarketStateSnapshot,
    embedding: Vec<f32>,
    point_id: u64,
) -> PointStruct {
    let git_sha = std::env::var("GIT_SHA").unwrap_or_else(|_| "dev".to_string());

    let date = chrono::DateTime::from_timestamp_millis(snapshot.timestamp as i64)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());

    // Calculate derived features for payload
    let ema_ratio = snapshot.ema_ratio_20_50();
    let oi_delta_pct = snapshot.oi_delta_pct();
    let volatility_ratio = if snapshot.atr_14_4h.abs() > 1e-9 {
        snapshot.atr_3_4h / snapshot.atr_14_4h
    } else {
        1.0
    };

    let payload_json = serde_json::json!({
        // Identification
        "symbol": snapshot.symbol,
        "timestamp": snapshot.timestamp,
        "price": snapshot.price,
        "date": date,

        // Indicators
        "rsi_7": snapshot.rsi_7,
        "rsi_14": snapshot.rsi_14,
        "macd": snapshot.macd,
        "ema_ratio": ema_ratio,

        // Derivatives
        "oi_delta_pct": oi_delta_pct,
        "funding_rate": snapshot.funding_rate,

        // Volatility
        "atr_3_4h": snapshot.atr_3_4h,
        "atr_14_4h": snapshot.atr_14_4h,
        "volatility_ratio": volatility_ratio,

        // Price changes
        "price_change_1h": snapshot.price_change_1h,
        "price_change_4h": snapshot.price_change_4h,

        // OUTCOMES - THE VALUABLE PART
        "outcome_15m": snapshot.outcome_15m,
        "outcome_1h": snapshot.outcome_1h,
        "outcome_4h": snapshot.outcome_4h,
        "outcome_24h": snapshot.outcome_24h,
        "max_runup_1h": snapshot.max_runup_1h,
        "max_drawdown_1h": snapshot.max_drawdown_1h,
        "hit_stop_loss": snapshot.hit_stop_loss,
        "hit_take_profit": snapshot.hit_take_profit,

        // Metadata & provenance
        "schema_version": 1,
        "feature_version": "v1_nofx_3m4h",
        "embedding_model": "bge-small-en-v1.5",
        "embedding_dim": 384,
        "build_id": git_sha,
    });

    // Convert to Map for Qdrant Payload compatibility
    let payload = payload_json.as_object().unwrap().clone();

    PointStruct::new(point_id, embedding, payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_to_point() {
        let mut snapshot = MarketStateSnapshot::new("BTCUSDT".to_string(), 1000000, 50000.0);
        snapshot.rsi_7 = 75.0;
        snapshot.ema_20_4h = 50500.0;
        snapshot.ema_50_4h = 50000.0;
        snapshot.outcome_4h = Some(-1.5);

        let embedding = vec![0.1; 384];
        let point = snapshot_to_point(&snapshot, embedding.clone(), 123);

        // Verify point is created with correct structure
        assert!(point.id.is_some());
        assert!(point.vectors.is_some());
        assert!(!point.payload.is_empty());

        // Verify payload contains expected fields
        assert!(point.payload.contains_key("symbol"));
        assert!(point.payload.contains_key("rsi_7"));
        assert!(point.payload.contains_key("outcome_4h"));
    }
}
