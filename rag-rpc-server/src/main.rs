mod protocol;
mod server;
mod handler;
mod config;
mod error;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::ServerConfig;
use server::RpcServer;

#[derive(Parser)]
#[command(name = "rag-rpc-server")]
#[command(about = "JSON-RPC server for RAG pattern retrieval")]
struct Cli {
    /// Server host to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Server port to bind to
    #[arg(long, default_value = "7879")]
    port: u16,

    /// Qdrant vector database URL
    #[arg(long, default_value = "http://localhost:6333")]
    qdrant_url: String,

    /// Qdrant collection name
    #[arg(long, default_value = "trading_patterns")]
    collection_name: String,

    /// Minimum number of matches required
    #[arg(long, default_value = "3")]
    min_matches: usize,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new(format!("rag_rpc_server={},trading_strategy={},trading_data_services={}",
                cli.log_level, cli.log_level, cli.log_level))
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🚀 RAG JSON-RPC Server Starting");
    tracing::info!("Configuration:");
    tracing::info!("  Host: {}", cli.host);
    tracing::info!("  Port: {}", cli.port);
    tracing::info!("  Qdrant URL: {}", cli.qdrant_url);
    tracing::info!("  Collection: {}", cli.collection_name);
    tracing::info!("  Min Matches: {}", cli.min_matches);

    let config = ServerConfig {
        host: cli.host,
        port: cli.port,
        qdrant_url: cli.qdrant_url,
        collection_name: cli.collection_name,
        min_matches: cli.min_matches,
    };

    let server = RpcServer::new(config).await?;
    server.run().await?;

    Ok(())
}
