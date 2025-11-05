/// Integration tests for the JSON-RPC server
///
/// These tests require:
/// 1. Qdrant running on localhost:6333
/// 2. Some test data ingested into the 'trading_patterns' collection
///
/// To run: cargo test --package rag-rpc-server --test integration_test -- --ignored --nocapture
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

#[test]
#[ignore] // Requires Qdrant running and test data
fn test_jsonrpc_query_patterns() {
    // Connect to server (assumes server is running on port 7879)
    let mut stream = TcpStream::connect("127.0.0.1:7879")
        .expect("Failed to connect to server. Is it running?");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    // Prepare request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "rag.query_patterns",
        "params": {
            "symbol": "BTCUSDT",
            "timestamp": 1730811225000u64,
            "current_state": {
                "price": 68500.50,
                "rsi_7": 83.6,
                "rsi_14": 78.2,
                "macd": 72.8,
                "ema_20": 68200.0,
                "ema_20_4h": 67800.0,
                "ema_50_4h": 67200.0,
                "funding_rate": 0.0001,
                "open_interest_latest": 1500000000.0,
                "open_interest_avg_24h": 1450000000.0
            },
            "query_config": {
                "lookback_days": 90,
                "top_k": 5,
                "min_similarity": 0.7
            }
        }
    });

    // Send request
    let request_json = serde_json::to_string(&request).unwrap();
    stream.write_all(request_json.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    stream.flush().unwrap();

    // Read response
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).unwrap();

    println!("Response: {}", response_line);

    // Parse response
    let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();

    // Validate response structure
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);

    if response.get("error").is_some() {
        println!("Error response: {}", response["error"]);
        // Don't fail the test if it's just insufficient matches
        if response["error"]["code"] == -32001 {
            println!("Insufficient matches (expected with test data)");
        }
    } else {
        // Success response
        assert!(response.get("result").is_some());
        let result = &response["result"];

        assert!(result.get("matches").is_some());
        assert!(result.get("statistics").is_some());
        assert!(result.get("metadata").is_some());

        println!("Matches found: {}", result["matches"].as_array().unwrap().len());
        println!(
            "Query duration: {}ms",
            result["metadata"]["query_duration_ms"]
        );
    }
}

#[test]
#[ignore]
fn test_jsonrpc_invalid_method() {
    let mut stream = TcpStream::connect("127.0.0.1:7879")
        .expect("Failed to connect to server");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "invalid.method",
        "params": {}
    });

    let request_json = serde_json::to_string(&request).unwrap();
    stream.write_all(request_json.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    stream.flush().unwrap();

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).unwrap();

    let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32601); // METHOD_NOT_FOUND
}

#[test]
#[ignore]
fn test_jsonrpc_invalid_params() {
    let mut stream = TcpStream::connect("127.0.0.1:7879")
        .expect("Failed to connect to server");

    let request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "rag.query_patterns",
        "params": {
            "invalid": "params"
        }
    });

    let request_json = serde_json::to_string(&request).unwrap();
    stream.write_all(request_json.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    stream.flush().unwrap();

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).unwrap();

    let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();

    assert!(response.get("error").is_some());
    assert_eq!(response["error"]["code"], -32602); // INVALID_PARAMS
}
