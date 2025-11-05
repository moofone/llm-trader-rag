use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Insufficient matches: found {found}, required {required}")]
    InsufficientMatches { found: usize, required: usize },

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Qdrant error: {0}")]
    QdrantError(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),
}

impl RpcError {
    /// Get the JSON-RPC error code for this error
    pub fn code(&self) -> i32 {
        use crate::protocol::*;
        match self {
            RpcError::ParseError(_) => PARSE_ERROR,
            RpcError::InvalidRequest(_) => INVALID_REQUEST,
            RpcError::MethodNotFound(_) => METHOD_NOT_FOUND,
            RpcError::InvalidParams(_) => INVALID_PARAMS,
            RpcError::InternalError(_) => INTERNAL_ERROR,
            RpcError::InsufficientMatches { .. } => INSUFFICIENT_MATCHES,
            RpcError::SymbolNotFound(_) => SYMBOL_NOT_FOUND,
            RpcError::QdrantError(_) => QDRANT_ERROR,
            RpcError::EmbeddingError(_) => EMBEDDING_ERROR,
        }
    }

    /// Get additional error data (optional)
    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            RpcError::InsufficientMatches { found, required } => Some(serde_json::json!({
                "matches_found": found,
                "min_required": required,
                "suggestion": "Try increasing lookback_days or reducing min_similarity"
            })),
            _ => None,
        }
    }
}

// Convert anyhow errors to RpcError
impl From<anyhow::Error> for RpcError {
    fn from(err: anyhow::Error) -> Self {
        RpcError::InternalError(err.to_string())
    }
}
