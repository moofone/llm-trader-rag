# Workflow-Manager Exploration Summary

## Findings Overview

Successfully explored the workflow-manager directory to understand JSON-RPC client structure, protocol format, node definitions, and JSON schema usage. All findings have been documented in the comprehensive guide.

---

## Key Discoveries

### 1. JSON-RPC 2.0 Protocol

The workflow-manager uses standard JSON-RPC 2.0 for communication:

**Request Format:**
```json
{
  "jsonrpc": "2.0",
  "id": <unique-id>,
  "method": "<method-name>",
  "params": <parameters>
}
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": <same-id>,
  "result": <result-data>
}
```

Or on error:
```json
{
  "jsonrpc": "2.0",
  "id": <same-id>,
  "error": { "code": -32601, "message": "Method not found" }
}
```

### 2. RPC Methods Identified

Two existing RPC methods were found:

1. **`ingest.push`** (Generic RPC Server)
   - Location: `/src/ingest/RPCServer.ts`
   - Endpoint: `POST /ingest`
   - Features: Schema validation, merge strategies, event callbacks, state management
   - Supports: idempotency keys, partial payloads, signature-based auth

2. **`snapshot.post`** (LLM Snapshot Intake)
   - Location: `/src/ingest/LlmSnapshotIntake.ts`
   - Endpoint: `POST /snapshot` or `POST /`
   - Features: Queue-based processing, deduplication, Zod schema validation
   - Supports: Derivatives enrichment, listener callbacks

### 3. JSON Schema Format

Schemas use JSON Schema Draft standard stored in `/schemas/`:

**Key Features:**
- Type validation (string, number, object, array, etc.)
- Required fields specification
- Constraints (minLength, minimum, pattern, enum)
- Nested object support
- `additionalProperties: false` for strict validation

**Example Files:**
- `schemas/llm-snapshot.schema.json` - Trading snapshot schema
- `schemas/workflow.schema.json` - Workflow definition schema
- `schemas/rag-query.schema.json` - (To be created for RAG)

### 4. Workflow Node Structure

Nodes are defined in YAML with this structure:

```yaml
nodes:
  <node-id>:
    component_type: tool|agent|llm|mcp|script|flow|map
    type: <specific-type>
    tool: <tool-name>
    params:
      <param-key>: <value>
    metadata:
      position: { x: <number>, y: <number> }
      notes: <string>
    outputs: [<output-names>]
```

**Component Types:**
- `agent` - Claude/LLM agents
- `llm` - Direct LLM calls
- `tool` - Tools (rpc_server, script_runner, etc.)
- `mcp` - Model Context Protocol tools
- `script` - JavaScript execution
- `flow` - Control flow
- `map` - Parallel processing

### 5. RPC Client Implementation

Simple, straightforward interface:

```typescript
async function sendRPCRequest(options: {
  host: string;
  port: number;
  method?: string;
  payload: any;
  timeout?: number;
}): Promise<{
  success: boolean;
  response?: any;
  error?: string;
  duration: number;
}>
```

Uses Node.js net.Socket for TCP connections.

### 6. RPC Server Startup Interface

```typescript
async function startRPCServer(options: {
  host?: string;              // Default: '127.0.0.1'
  port?: number;              // Default: 7878
  secret?: string;            // HMAC-SHA256 signatures
  eventName?: string;         // State variable name
  schemaPath?: string;        // JSON schema file path
  validateSchema?: boolean;   // Default: true
  mergeStrategy?: 'overwrite'|'shallow'|'deep'|'append';
  allowPartial?: boolean;     // Partial payload support
  onIncomingData?: callback;  // Event listener
  eventMaxAgeMs?: number;     // Queue max age
  saveEvents?: boolean;       // Persist to disk
}): Promise<ServerHandle>
```

---

## File Locations

### Core RPC Implementation
- **RPC Server:** `/workflow-manager/src/ingest/RPCServer.ts`
- **RPC Client:** `/workflow-manager/src/ingest/RPCClient.ts`
- **Snapshot Intake:** `/workflow-manager/src/ingest/LlmSnapshotIntake.ts`

### Schema Definitions
- **Schemas Directory:** `/workflow-manager/schemas/`
- **Schema Generator:** `/workflow-manager/src/schema-generator.ts`

### Workflow System
- **DAG Types:** `/workflow-manager/src/dag/types.ts`
- **DAG Schema:** `/workflow-manager/src/dag/schema.ts`
- **Node Options:** `/workflow-manager/src/workflow/nodeOptionSpecs.ts`

### Example Workflows
- **Trading Bot:** `/workflow-manager/workflows/llm_trading_bot/event-driven-trading.yaml`

---

## Implementation Patterns

### RPC Server Pattern

1. Create HTTP server with `createServer()`
2. Listen on POST endpoint (e.g., `/ingest`, `/snapshot`)
3. Parse JSON-RPC request
4. Validate `jsonrpc === '2.0'` and method name
5. Extract params and validate against schema
6. Process request and return result
7. Wrap response in JSON-RPC envelope

### RPC Client Pattern

1. Create socket connection to host:port
2. Build JSON-RPC request object
3. Serialize to JSON and send with newline delimiter
4. Read response data
5. Parse JSON response
6. Return success/error

### Workflow Node Pattern

1. Define node in YAML with component_type and type
2. Specify params for node configuration
3. Add metadata for UI positioning
4. Define inputs/outputs for data flow
5. Register schema in `src/dag/schema.ts` (Zod)
6. Define UI options in `src/workflow/nodeOptionSpecs.ts`

---

## For RAG Integration

To implement RAG query as a JSON-RPC endpoint:

1. **Create Schema:** `schemas/rag-query.schema.json`
   - Define query structure
   - Define context fields
   - Specify max_results constraints

2. **Implement Server:** RAG HTTP server listening on port 8000
   - Handle `POST /query` endpoint
   - Support JSON-RPC 2.0 method: `rag.query`
   - Validate params against schema
   - Return standardized JSON-RPC responses

3. **Define Workflow Node:** Add to workflow YAML
   - component_type: tool
   - type: rag_query
   - Configure host, port, schema path
   - Set merge strategy and event name

4. **Register in System:** Update `src/dag/schema.ts`
   - Add Zod schema for RAG node
   - Define validation rules

5. **Add UI Options:** Update `src/workflow/nodeOptionSpecs.ts`
   - Host/port configuration
   - Schema path input
   - Merge strategy selector

---

## Test Commands

### Test RPC Server Health
```bash
curl http://127.0.0.1:8000/health
```

### Test RPC Method Call
```bash
curl -X POST http://127.0.0.1:8000/query \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "rag.query",
    "params": {
      "query": "What are support levels for BTC?"
    }
  }'
```

### Using RPC Client
```typescript
const result = await sendRPCRequest({
  host: '127.0.0.1',
  port: 8000,
  method: 'rag.query',
  payload: {
    query: 'market analysis',
    max_results: 5
  },
  timeout: 10000
});
```

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│ Workflow (YAML)                                         │
│  - nodes: { RAG Query Service, ... }                    │
│  - edges: [ { from, to, data mappings } ]               │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│ Workflow Runtime (GraphTheoryWorkflowRuntime)           │
│  - DAG execution engine                                 │
│  - Node execution & data flow                           │
│  - State management                                     │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│ RPC Client (sendRPCRequest)                             │
│  - TCP socket connection                                │
│  - JSON-RPC 2.0 serialization                           │
└────────────────────┬────────────────────────────────────┘
                     │
         ┌───────────┴───────────┐
         │                       │
         ▼                       ▼
   ┌──────────────────┐  ┌──────────────────┐
   │ RPCServer        │  │ RAGQueryServer   │
   │ (Generic Ingest) │  │ (Your Service)   │
   │ Port: 7878       │  │ Port: 8000       │
   └──────────────────┘  └──────────────────┘
         │                       │
         ▼                       ▼
   ┌──────────────────┐  ┌──────────────────┐
   │ Schema Valid.    │  │ RAG Processing   │
   │ Event Callbacks  │  │ Context Search   │
   │ State Merging    │  │ Result Ranking   │
   └──────────────────┘  └──────────────────┘
```

---

## Documentation Generated

A comprehensive guide has been created: `/Users/greg/Dev/git/llm-trader-rag/WORKFLOW_MANAGER_RPC_GUIDE.md`

This document contains:
1. Complete JSON-RPC 2.0 protocol format
2. Detailed explanation of existing RPC methods
3. JSON schema format and examples
4. Workflow node definition guide
5. RPC client implementation details
6. Step-by-step guide to define new nodes
7. Complete RAG implementation example
8. Integration examples with workflows
9. Test commands and cURL examples

---

## Next Steps

1. **Review the generated guide** to understand the full context
2. **Create RAG schema** based on your query structure
3. **Implement RAG JSON-RPC server** following the minimal example
4. **Define workflow node** in your trading bot workflow
5. **Register in workflow-manager** following the patterns shown
6. **Test integration** with the provided test commands
7. **Iterate** based on your specific RAG requirements

---

## Key Takeaways

- **JSON-RPC 2.0** is the standard protocol - simple, well-documented
- **Two existing implementations** to learn from (generic ingest, snapshot intake)
- **Schema validation** is strongly recommended but optional
- **Event callbacks** enable real-time data flow to workflow engine
- **Merge strategies** provide flexible data combination options
- **Workflow nodes** are declarative YAML configurations
- **RPC client** is lightweight, using TCP sockets
- **Extensible system** - easy to add new RPC methods and node types

