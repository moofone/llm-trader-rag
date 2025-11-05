use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Parser;
use trading_data_services::HistoricalIngestionPipeline;
use tracing::{info, Level};
use tracing_subscriber;

/// RAG Historical Data Ingestion CLI
///
/// Extracts historical market snapshots from LMDB, converts to embeddings,
/// and uploads to Qdrant vector database for RAG-enhanced trading signals.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Trading symbols to ingest (comma-separated)
    #[arg(short, long, value_delimiter = ',', default_value = "BTCUSDT,ETHUSDT")]
    symbols: Vec<String>,

    /// Start date (RFC3339 format or days ago)
    /// Examples: "2025-10-01T00:00:00Z" or "90" (for 90 days ago)
    #[arg(short = 's', long, default_value = "90")]
    start: String,

    /// End date (RFC3339 format or "now")
    #[arg(short = 'e', long, default_value = "now")]
    end: String,

    /// Snapshot interval in minutes
    #[arg(short = 'i', long, default_value = "15")]
    interval: u64,

    /// Qdrant URL
    #[arg(short = 'q', long, default_value = "http://localhost:6333")]
    qdrant_url: String,

    /// Qdrant collection name
    #[arg(short = 'c', long, default_value = "trading_patterns")]
    collection: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short = 'l', long, default_value = "info")]
    log_level: String,
}

impl Args {
    /// Parse start timestamp from string (either RFC3339 date or days ago)
    fn parse_start_timestamp(&self) -> Result<u64> {
        // Try parsing as number of days
        if let Ok(days_ago) = self.start.parse::<u64>() {
            let timestamp = Utc::now().timestamp_millis() as u64 - (days_ago * 24 * 60 * 60 * 1000);
            return Ok(timestamp);
        }

        // Try parsing as RFC3339 date
        let dt = DateTime::parse_from_rfc3339(&self.start)?;
        Ok(dt.timestamp_millis() as u64)
    }

    /// Parse end timestamp from string (either RFC3339 date or "now")
    fn parse_end_timestamp(&self) -> Result<u64> {
        if self.end == "now" {
            return Ok(Utc::now().timestamp_millis() as u64);
        }

        let dt = DateTime::parse_from_rfc3339(&self.end)?;
        Ok(dt.timestamp_millis() as u64)
    }

    /// Parse log level from string
    fn parse_log_level(&self) -> Level {
        match self.log_level.to_lowercase().as_str() {
            "trace" => Level::TRACE,
            "debug" => Level::DEBUG,
            "info" => Level::INFO,
            "warn" => Level::WARN,
            "error" => Level::ERROR,
            _ => Level::INFO,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(args.parse_log_level())
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("🚀 RAG Historical Data Ingestion Tool");
    info!("=====================================");

    // Parse timestamps
    let start_ts = args.parse_start_timestamp()?;
    let end_ts = args.parse_end_timestamp()?;

    let start_date = DateTime::from_timestamp_millis(start_ts as i64)
        .unwrap()
        .to_rfc3339();
    let end_date = DateTime::from_timestamp_millis(end_ts as i64)
        .unwrap()
        .to_rfc3339();

    info!("Configuration:");
    info!("  Symbols: {:?}", args.symbols);
    info!("  Start: {} ({})", start_date, start_ts);
    info!("  End: {} ({})", end_date, end_ts);
    info!("  Interval: {} minutes", args.interval);
    info!("  Qdrant URL: {}", args.qdrant_url);
    info!("  Collection: {}", args.collection);
    info!("");

    // Create ingestion pipeline
    info!("Initializing ingestion pipeline...");
    let mut pipeline =
        HistoricalIngestionPipeline::new(&args.qdrant_url, args.collection).await?;

    info!("Pipeline initialized successfully");
    info!("");

    // Ingest all symbols
    let symbol_refs: Vec<&str> = args.symbols.iter().map(|s| s.as_str()).collect();
    let results = pipeline
        .ingest_multiple_symbols(symbol_refs, start_ts, end_ts, args.interval)
        .await?;

    // Display results
    info!("");
    info!("✅ Ingestion Complete!");
    info!("=====================");
    for (symbol, stats) in results {
        info!(
            "  {}: {} snapshots, {} embeddings, {} points uploaded",
            symbol, stats.snapshots_created, stats.embeddings_generated, stats.points_uploaded
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_days_ago() {
        let args = Args {
            symbols: vec![],
            start: "90".to_string(),
            end: "now".to_string(),
            interval: 15,
            qdrant_url: "".to_string(),
            collection: "".to_string(),
            log_level: "info".to_string(),
        };

        let start_ts = args.parse_start_timestamp().unwrap();
        let now = Utc::now().timestamp_millis() as u64;
        let expected = now - 90 * 24 * 60 * 60 * 1000;

        // Allow 1 second tolerance
        assert!((start_ts as i64 - expected as i64).abs() < 1000);
    }

    #[test]
    fn test_parse_rfc3339() {
        let args = Args {
            symbols: vec![],
            start: "2025-10-01T00:00:00Z".to_string(),
            end: "2025-11-01T00:00:00Z".to_string(),
            interval: 15,
            qdrant_url: "".to_string(),
            collection: "".to_string(),
            log_level: "info".to_string(),
        };

        let start_ts = args.parse_start_timestamp().unwrap();
        let end_ts = args.parse_end_timestamp().unwrap();

        assert!(end_ts > start_ts);
    }
}
