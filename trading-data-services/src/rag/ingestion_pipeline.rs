use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;
use trading_core::TimestampMS;
use tracing;

use super::snapshot_extractor::HistoricalSnapshotExtractor;
use super::snapshot_formatter::SnapshotFormatter;
use super::vector_store::{snapshot_to_point, VectorStore};

/// Statistics from an ingestion run
#[derive(Debug, Default, Clone)]
pub struct IngestStats {
    pub snapshots_created: usize,
    pub embeddings_generated: usize,
    pub points_uploaded: usize,
}

/// Historical ingestion pipeline that:
/// 1. Extracts snapshots from LMDB
/// 2. Converts to natural language
/// 3. Generates embeddings
/// 4. Uploads to Qdrant
pub struct HistoricalIngestionPipeline {
    snapshot_extractor: Arc<HistoricalSnapshotExtractor>,
    embedding_model: TextEmbedding,
    vector_store: Arc<VectorStore>,
}

impl HistoricalIngestionPipeline {
    /// Create a new ingestion pipeline
    pub async fn new(qdrant_url: &str, collection_name: String) -> Result<Self> {
        tracing::info!("Initializing ingestion pipeline...");

        // Initialize embedding model (downloads BGE model on first run)
        tracing::info!("Loading embedding model (BGE-small-en-v1.5)...");
        let embedding_model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true),
        )?;

        // Initialize snapshot extractor
        let snapshot_extractor = Arc::new(HistoricalSnapshotExtractor::new());

        // Initialize vector store
        let vector_store = Arc::new(VectorStore::new(qdrant_url, collection_name).await?);

        // Create collection if it doesn't exist (BGE-small uses 384 dimensions)
        vector_store.create_collection_if_not_exists(384).await?;

        tracing::info!("Ingestion pipeline initialized successfully");

        Ok(Self {
            snapshot_extractor,
            embedding_model,
            vector_store,
        })
    }

    /// Ingest all historical data for a symbol
    pub async fn ingest_symbol_history(
        &mut self,
        symbol: &str,
        start_timestamp: TimestampMS,
        end_timestamp: TimestampMS,
        snapshot_interval_minutes: u64,
    ) -> Result<IngestStats> {
        let mut stats = IngestStats::default();

        let start_date = chrono::DateTime::from_timestamp_millis(start_timestamp as i64)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string());

        let end_date = chrono::DateTime::from_timestamp_millis(end_timestamp as i64)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string());

        tracing::info!(
            "Starting ingestion for {} from {} to {} (every {} minutes)",
            symbol,
            start_date,
            end_date,
            snapshot_interval_minutes
        );

        // Step 1: Extract snapshots from LMDB
        let snapshots = self.snapshot_extractor.extract_snapshots(
            symbol,
            start_timestamp,
            end_timestamp,
            snapshot_interval_minutes,
        )?;

        stats.snapshots_created = snapshots.len();
        tracing::info!("Created {} snapshots for {}", snapshots.len(), symbol);

        if snapshots.is_empty() {
            tracing::warn!("No snapshots created for {}", symbol);
            return Ok(stats);
        }

        // Step 2: Generate embeddings in batches
        const BATCH_SIZE: usize = 100;
        let mut all_points = Vec::new();
        let mut point_id = 0u64;

        for batch in snapshots.chunks(BATCH_SIZE) {
            // Convert to text
            let texts: Vec<String> = batch.iter().map(|s| s.to_embedding_text()).collect();

            tracing::info!(
                "Generating embeddings for batch of {} snapshots...",
                texts.len()
            );

            // Generate embeddings (much faster in batch)
            let embeddings = self.embedding_model.embed(texts, None)?;
            stats.embeddings_generated += embeddings.len();

            // Create Qdrant points
            for (snapshot, embedding) in batch.iter().zip(embeddings.iter()) {
                let point = snapshot_to_point(snapshot, embedding.clone(), point_id);
                all_points.push(point);
                point_id += 1;
            }

            tracing::info!(
                "Processed {} embeddings (total: {})",
                embeddings.len(),
                stats.embeddings_generated
            );
        }

        // Step 3: Upload to Qdrant
        if !all_points.is_empty() {
            tracing::info!("Uploading {} points to Qdrant...", all_points.len());
            self.vector_store.upsert_points(all_points).await?;
            stats.points_uploaded = point_id as usize;
            tracing::info!("Uploaded {} points to Qdrant", stats.points_uploaded);
        }

        tracing::info!("Ingestion complete for {}: {:?}", symbol, stats);
        Ok(stats)
    }

    /// Ingest multiple symbols
    pub async fn ingest_multiple_symbols(
        &mut self,
        symbols: Vec<&str>,
        start_timestamp: TimestampMS,
        end_timestamp: TimestampMS,
        snapshot_interval_minutes: u64,
    ) -> Result<Vec<(String, IngestStats)>> {
        let mut results = Vec::new();

        for symbol in symbols {
            tracing::info!("Processing symbol: {}", symbol);
            let stats = self
                .ingest_symbol_history(
                    symbol,
                    start_timestamp,
                    end_timestamp,
                    snapshot_interval_minutes,
                )
                .await?;
            results.push((symbol.to_string(), stats));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Qdrant running
    async fn test_ingestion_pipeline() {
        let mut pipeline = HistoricalIngestionPipeline::new(
            "http://localhost:6333",
            "test_trading_patterns".to_string(),
        )
        .await
        .unwrap();

        let start = chrono::Utc::now().timestamp_millis() as u64 - 24 * 60 * 60 * 1000; // 24h ago
        let end = chrono::Utc::now().timestamp_millis() as u64;

        let stats = pipeline
            .ingest_symbol_history("BTCUSDT", start, end, 15)
            .await
            .unwrap();

        assert!(stats.snapshots_created > 0);
        assert_eq!(stats.embeddings_generated, stats.snapshots_created);
        assert_eq!(stats.points_uploaded, stats.snapshots_created);
    }
}
