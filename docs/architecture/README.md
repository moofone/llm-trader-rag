# Architecture Documentation

This directory contains comprehensive architecture documentation for the llm-trader-rag system.

## 📚 Documentation Index

### For Integrators (Start Here)

1. **[API_QUICK_START.md](./API_QUICK_START.md)** ⚡
   - Quick reference for getting started
   - Minimal examples
   - Common error codes
   - Node.js integration example
   - **Read this first if you're integrating with workflow-manager**

2. **[jsonrpc_api.md](./jsonrpc_api.md)** 📖
   - Complete JSON-RPC 2.0 API specification
   - Detailed parameter documentation
   - Request/response schemas
   - Error handling guide
   - Workflow integration examples
   - JSON Schema definitions
   - **Reference this for complete API details**

### For Architects & Developers

3. **[architecture.md](./architecture.md)** 🏗️
   - Complete system architecture overview
   - Service responsibilities
   - Data flow diagrams
   - Integration with other services
   - Design rationale
   - **Read this to understand the big picture**

## Quick Overview

### What is llm-trader-rag?

A **Retrieval-Augmented Generation (RAG)** service that provides historical pattern matching for trading decisions.

**Input:** Current market state (RSI, MACD, EMAs, funding, OI, etc.)
**Process:** Semantic similarity search using vector embeddings
**Output:** Historical matches + outcomes + statistics

### Key Components

```
┌─────────────────────────────────────────────────┐
│  JSON-RPC Server (TCP port 7879)                │
│  • Method: rag.query_patterns                   │
│  • Protocol: JSON-RPC 2.0                       │
│  • Transport: TCP socket                        │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│  RAG Pipeline                                   │
│  1. Generate embedding (FastEmbed)              │
│  2. Search Qdrant vector DB                     │
│  3. Calculate statistics                        │
│  4. Return matches + metadata                   │
└─────────────────────────────────────────────────┘
```

## Integration Flow

```
workflow-manager
      │
      │ 1. Receives market data from llm-trader-data
      │
      ▼
   [Format Request]
      │
      │ 2. JSON-RPC call to rag-rpc-server
      │
      ▼
 rag-rpc-server (this service)
      │
      │ 3. Returns historical matches
      │
      ▼
   [Format Prompt]
      │
      │ 4. Send to LLM with RAG context
      │
      ▼
  LLM Decision
```

## Status

✅ **Phase 1:** Historical Data Ingestion - COMPLETE
✅ **Phase 2:** Live Pattern Retrieval - COMPLETE
✅ **Phase 3:** LLM Client Integration - COMPLETE
✅ **Phase 4:** JSON-RPC Server - COMPLETE
📋 **Phase 5:** workflow-manager Integration - PENDING (client-side work)

## Key Files

| File | Purpose |
|------|---------|
| `API_QUICK_START.md` | Fast integration guide |
| `jsonrpc_api.md` | Complete API specification |
| `architecture.md` | System architecture overview |

## Related Documentation

- **Main README:** [../../README.md](../../README.md)
- **Project Status:** [../../PROJECT_STATUS.md](../../PROJECT_STATUS.md)
- **What's Next:** [../../WHATS_NEXT.md](../../WHATS_NEXT.md)
- **Deployment Guide:** [../../DEPLOYMENT_GUIDE.md](../../DEPLOYMENT_GUIDE.md)
- **Server README:** [../../rag-rpc-server/README.md](../../rag-rpc-server/README.md)

## Quick Start

```bash
# 1. Start Qdrant
docker run -d -p 6333:6333 --name qdrant qdrant/qdrant

# 2. Ingest data
cargo run --release --bin rag-ingest -- --symbols BTCUSDT,ETHUSDT --start 90 --end now

# 3. Start server
cargo run --release --bin rag-rpc-server

# 4. Test
./rag-rpc-server/test_request.sh
```

## Support

- **Questions:** See [API_QUICK_START.md](./API_QUICK_START.md) troubleshooting section
- **API Details:** See [jsonrpc_api.md](./jsonrpc_api.md)
- **Architecture:** See [architecture.md](./architecture.md)
- **Test Script:** Run `../../rag-rpc-server/test_request.sh`

---

**Last Updated:** 2025-11-05
**Status:** ✅ Production Ready
