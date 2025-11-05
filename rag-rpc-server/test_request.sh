#!/bin/bash
# Test the RAG JSON-RPC server with netcat
#
# Usage: ./test_request.sh [port]
# Default port: 7879

PORT=${1:-7879}

echo "Testing RAG JSON-RPC server on localhost:$PORT"
echo ""

# Test 1: Valid query
echo "Test 1: Valid rag.query_patterns request"
echo '{"jsonrpc":"2.0","id":1,"method":"rag.query_patterns","params":{"symbol":"BTCUSDT","timestamp":1730811225000,"current_state":{"price":68500.50,"rsi_7":83.6,"rsi_14":78.2,"macd":72.8,"ema_20":68200.0,"ema_20_4h":67800.0,"ema_50_4h":67200.0,"funding_rate":0.0001,"open_interest_latest":1500000000.0,"open_interest_avg_24h":1450000000.0},"query_config":{"lookback_days":90,"top_k":5,"min_similarity":0.7}}}' | nc localhost $PORT
echo ""
echo "---"
echo ""

# Test 2: Invalid method
echo "Test 2: Invalid method (should return -32601)"
echo '{"jsonrpc":"2.0","id":2,"method":"invalid.method","params":{}}' | nc localhost $PORT
echo ""
echo "---"
echo ""

# Test 3: Missing params
echo "Test 3: Missing params (should return -32602)"
echo '{"jsonrpc":"2.0","id":3,"method":"rag.query_patterns"}' | nc localhost $PORT
echo ""
echo "---"
echo ""

# Test 4: Malformed JSON
echo "Test 4: Malformed JSON (should return -32700)"
echo '{invalid json}' | nc localhost $PORT
echo ""
echo "---"
echo ""

# Test 5: Query with minimal params (uses defaults)
echo "Test 5: Query with minimal config (uses defaults)"
echo '{"jsonrpc":"2.0","id":5,"method":"rag.query_patterns","params":{"symbol":"BTCUSDT","timestamp":1730811225000,"current_state":{"price":68500.50,"rsi_7":83.6,"rsi_14":78.2,"macd":72.8,"ema_20":68200.0,"ema_20_4h":67800.0,"ema_50_4h":67200.0,"funding_rate":0.0001,"open_interest_latest":1500000000.0,"open_interest_avg_24h":1450000000.0}}}' | nc localhost $PORT
echo ""

echo ""
echo "All tests completed!"
