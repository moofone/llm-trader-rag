use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Success Response
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Value,
}

/// JSON-RPC 2.0 Error Response
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub error: ErrorObject,
}

/// JSON-RPC Error Object
#[derive(Debug, Serialize)]
pub struct ErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// Custom error codes for RAG operations
pub const INSUFFICIENT_MATCHES: i32 = -32001;
pub const SYMBOL_NOT_FOUND: i32 = -32002;
pub const QDRANT_ERROR: i32 = -32003;
pub const EMBEDDING_ERROR: i32 = -32004;

/// RAG Query Request Parameters
#[derive(Debug, Deserialize)]
pub struct RagQueryRequest {
    pub symbol: String,
    pub timestamp: u64,
    pub current_state: MarketState,
    #[serde(default)]
    pub query_config: QueryConfig,
}

/// Market state from workflow-manager
#[derive(Debug, Deserialize)]
pub struct MarketState {
    pub price: f64,
    pub rsi_7: f64,
    pub rsi_14: f64,
    pub macd: f64,
    pub ema_20: f64,
    pub ema_20_4h: f64,
    pub ema_50_4h: f64,
    pub funding_rate: f64,
    pub open_interest_latest: f64,
    pub open_interest_avg_24h: f64,
    pub price_change_1h: Option<f64>,
    pub price_change_4h: Option<f64>,
}

/// Query configuration with defaults
#[derive(Debug, Deserialize)]
pub struct QueryConfig {
    #[serde(default = "default_lookback_days")]
    pub lookback_days: u32,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
    #[serde(default = "default_include_regime_filters")]
    pub include_regime_filters: bool,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            lookback_days: default_lookback_days(),
            top_k: default_top_k(),
            min_similarity: default_min_similarity(),
            include_regime_filters: default_include_regime_filters(),
        }
    }
}

fn default_lookback_days() -> u32 {
    90
}
fn default_top_k() -> usize {
    5
}
fn default_min_similarity() -> f32 {
    0.7
}
fn default_include_regime_filters() -> bool {
    true
}

/// RAG Query Response
#[derive(Debug, Serialize)]
pub struct RagQueryResponse {
    pub matches: Vec<HistoricalMatchJson>,
    pub statistics: Statistics,
    pub metadata: Metadata,
}

/// Historical match in JSON format
#[derive(Debug, Serialize)]
pub struct HistoricalMatchJson {
    pub similarity: f32,
    pub timestamp: u64,
    pub date: String,
    pub market_state: MatchMarketState,
    pub outcomes: Outcomes,
}

/// Market state of a historical match
#[derive(Debug, Serialize)]
pub struct MatchMarketState {
    pub rsi_7: f64,
    pub rsi_14: f64,
    pub macd: f64,
    pub ema_ratio: f64,
    pub oi_delta_pct: f64,
    pub funding_rate: f64,
}

/// Outcomes after the historical match
#[derive(Debug, Serialize)]
pub struct Outcomes {
    pub outcome_1h: Option<f64>,
    pub outcome_4h: Option<f64>,
    pub outcome_24h: Option<f64>,
    pub max_runup_1h: Option<f64>,
    pub max_drawdown_1h: Option<f64>,
    pub hit_stop_loss: Option<bool>,
    pub hit_take_profit: Option<bool>,
}

/// Statistics across all matches
#[derive(Debug, Serialize)]
pub struct Statistics {
    pub total_matches: usize,
    pub avg_similarity: f32,
    pub similarity_range: [f32; 2],
    pub outcome_4h: OutcomeStats,
    pub stop_loss_hits: usize,
    pub take_profit_hits: usize,
}

/// Statistical outcomes
#[derive(Debug, Serialize)]
pub struct OutcomeStats {
    pub mean: f64,
    pub median: f64,
    pub p10: f64,
    pub p90: f64,
    pub positive_count: usize,
    pub negative_count: usize,
    pub win_rate: f64,
}

/// Query metadata
#[derive(Debug, Serialize)]
pub struct Metadata {
    pub query_duration_ms: u64,
    pub embedding_duration_ms: u64,
    pub retrieval_duration_ms: u64,
    pub filters_applied: Vec<String>,
    pub schema_version: u32,
    pub feature_version: String,
    pub embedding_model: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_config_defaults() {
        let config: QueryConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.lookback_days, 90);
        assert_eq!(config.top_k, 5);
        assert_eq!(config.min_similarity, 0.7);
        assert_eq!(config.include_regime_filters, true);
    }

    #[test]
    fn test_parse_jsonrpc_request() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "method": "rag.query_patterns",
            "params": {}
        }"#;

        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "rag.query_patterns");
    }
}
