/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub qdrant_url: String,
    pub collection_name: String,
    pub min_matches: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 7879,
            qdrant_url: "http://localhost:6333".to_string(),
            collection_name: "trading_patterns".to_string(),
            min_matches: 3,
        }
    }
}
