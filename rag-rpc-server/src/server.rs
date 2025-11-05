use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use trading_data_services::VectorStore;
use trading_strategy::llm::RagRetriever;

use crate::config::ServerConfig;
use crate::error::RpcError;
use crate::handler::RagQueryHandler;
use crate::protocol::*;

/// JSON-RPC server for RAG queries
pub struct RpcServer {
    config: ServerConfig,
    handler: Arc<RagQueryHandler>,
}

impl RpcServer {
    /// Create a new RPC server
    pub async fn new(config: ServerConfig) -> Result<Self> {
        tracing::info!("Initializing RAG components...");

        // Initialize vector store
        let vector_store = Arc::new(
            VectorStore::new(&config.qdrant_url, config.collection_name.clone())
                .await
                .context("Failed to connect to Qdrant")?,
        );

        // Initialize RAG retriever
        let retriever = Arc::new(
            RagRetriever::new(vector_store, config.min_matches)
                .await
                .context("Failed to initialize RAG retriever")?,
        );

        let handler = Arc::new(RagQueryHandler::new(retriever, config.min_matches));

        tracing::info!("✅ RAG components initialized successfully");

        Ok(Self { config, handler })
    }

    /// Start the server and handle connections
    pub async fn run(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .context(format!("Failed to bind to {}", addr))?;

        tracing::info!("✅ RAG JSON-RPC Server listening on {}", addr);
        tracing::info!("Ready to accept connections");

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    tracing::debug!("New connection from {}", addr);
                    let handler = Arc::clone(&self.handler);

                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, handler).await {
                            tracing::error!("Connection error from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Failed to accept connection: {}", e);
                }
            }
        }
    }
}

/// Handle a single TCP connection
async fn handle_connection(
    mut socket: TcpStream,
    handler: Arc<RagQueryHandler>,
) -> Result<()> {
    let (reader, mut writer) = socket.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            // Connection closed
            break;
        }

        tracing::debug!("Received request: {}", line.trim());

        // Process JSON-RPC request
        let response = process_request(&line, &handler).await;

        // Send response
        let response_json = serde_json::to_string(&response)?;
        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        tracing::debug!("Sent response");
    }

    Ok(())
}

/// Process a JSON-RPC request
async fn process_request(
    line: &str,
    handler: &RagQueryHandler,
) -> serde_json::Value {
    // Parse JSON-RPC request
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(req) => req,
        Err(e) => {
            return serde_json::to_value(JsonRpcError {
                jsonrpc: "2.0".to_string(),
                id: None,
                error: ErrorObject {
                    code: PARSE_ERROR,
                    message: format!("Parse error: {}", e),
                    data: None,
                },
            })
            .unwrap();
        }
    };

    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return create_error_response(
            request.id,
            RpcError::InvalidRequest("JSON-RPC version must be 2.0".to_string()),
        );
    }

    // Route to method handler
    match request.method.as_str() {
        "rag.query_patterns" => handle_query_patterns(request, handler).await,
        _ => create_error_response(
            request.id,
            RpcError::MethodNotFound(request.method.clone()),
        ),
    }
}

/// Handle rag.query_patterns method
async fn handle_query_patterns(
    request: JsonRpcRequest,
    handler: &RagQueryHandler,
) -> Value {
    // Parse params
    let params: RagQueryRequest = match request.params {
        Some(params) => match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => {
                return create_error_response(
                    request.id,
                    RpcError::InvalidParams(format!("Invalid params: {}", e)),
                );
            }
        },
        None => {
            return create_error_response(
                request.id,
                RpcError::InvalidParams("Missing params".to_string()),
            );
        }
    };

    // Handle query
    match handler.handle_query(params).await {
        Ok(result) => {
            // Success response
            serde_json::to_value(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: serde_json::to_value(result).unwrap(),
            })
            .unwrap()
        }
        Err(e) => create_error_response(request.id, e),
    }
}

/// Create an error response
fn create_error_response(id: Option<Value>, error: RpcError) -> Value {
    serde_json::to_value(JsonRpcError {
        jsonrpc: "2.0".to_string(),
        id,
        error: ErrorObject {
            code: error.code(),
            message: error.to_string(),
            data: error.data(),
        },
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_error_response() {
        let error = RpcError::MethodNotFound("test.method".to_string());
        let response = create_error_response(Some(Value::from(1)), error);

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Method not found"));
        assert!(json.contains("-32601"));
    }
}
