# Workflow-Manager: JSON-RPC Protocol and Workflow Node Integration Guide

## Overview

The workflow-manager uses JSON-RPC 2.0 protocol for communication with event-driven servers. This guide explains the RPC protocol format, how nodes are defined, how to register RPC methods, and how to implement a new workflow node for the RAG query service.

---

## 1. JSON-RPC 2.0 Protocol Format

### Request Structure

The standard JSON-RPC 2.0 request format:

```typescript
{
  "jsonrpc": "2.0",
  "id": <unique-identifier>,
  "method": "<method-name>",
  "params": <parameters-object-or-array>
}
```

### Response Structure

Successful response:
```typescript
{
  "jsonrpc": "2.0",
  "id": <same-id-as-request>,
  "result": <result-data>
}
```

Error response:
```typescript
{
  "jsonrpc": "2.0",
  "id": <same-id-as-request>,
  "error": {
    "code": <error-code>,
    "message": "<error-message>"
  }
}
```

### Common Error Codes

- `-32601`: Method not found
- `-32602`: Invalid params
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32603`: Internal error

### Example Request/Response

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ingest.push",
  "params": {
    "channel": "price_feed",
    "payload": {
      "symbol": "BTC",
      "price": 45000
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "id": "uuid-123",
    "channel": "price_feed",
    "stored": true,
    "path": "var/ingest/price_feed/20250105/uuid-123.json"
  }
}
```

---

## 2. Existing RPC Methods

### 2.1 `ingest.push` (Generic RPC Server)

**Location:** `/Users/greg/Dev/git/workflow-manager/src/ingest/RPCServer.ts`

**Purpose:** Generic event ingestion endpoint for arbitrary data with schema validation

**Method Name:** `ingest.push`

**Endpoint:** `POST /ingest`

**Request Format:**
```typescript
{
  "jsonrpc": "2.0",
  "id": <number>,
  "method": "ingest.push",
  "params": {
    "channel": string;              // Required: Event channel name
    "payload": object;              // Required: Event payload
    "version": string;              // Optional: Version (default: "1")
    "id": string;                   // Optional: Custom event ID
    "source": string;               // Optional: Source identifier
    "tags": string[];               // Optional: Tags for filtering
    "schemaUri": string;            // Optional: Schema URI reference
    "idempotencyKey": string;       // Optional: For deduplication
    "createdAt": string;            // Optional: ISO timestamp
    "mappings": {                   // Optional: JSONPath mappings
      state?: Array<{
        key: string;
        jsonPath: string;
      }>;
      variables?: Array<{
        key: string;
        jsonPath: string;
      }>;
    };
    "apply": {                      // Optional: State patch operations
      stateKey?: string;
      path?: string;
      op?: 'merge' | 'append' | 'replace' | 'json_patch' | 'json_merge_patch';
      primaryKey?: string;
      limit?: number;
    };
  }
}
```

**Response:**
```typescript
{
  "jsonrpc": "2.0",
  "id": <number>,
  "result": {
    "id": string;                   // Generated event ID
    "channel": string;              // Event channel
    "stored": boolean;              // Whether data was stored
    "path": string;                 // File path where stored
  }
}
```

**Features:**
- Schema validation support (optional)
- Multiple merge strategies (overwrite, shallow, deep, append)
- Partial payload support (allowPartial)
- Event callbacks on incoming data
- Signature-based authentication support
- Event persistence to disk

---

### 2.2 `snapshot.post` (LLM Snapshot Intake)

**Location:** `/Users/greg/Dev/git/workflow-manager/src/ingest/LlmSnapshotIntake.ts`

**Purpose:** Specialized endpoint for trading snapshot data with schema validation

**Method Name:** `snapshot.post`

**Endpoint:** `POST /snapshot` or `POST /`

**Request Format:**
```typescript
{
  "jsonrpc": "2.0",
  "id": <number>,
  "method": "snapshot.post",
  "params": {
    "job_id": string;               // Required: Job identifier
    "snapshot_id": string;          // Required: Snapshot identifier
    "deadline_ms": number;          // Required: Deadline in milliseconds
    "snapshot": {
      "risk_controls": {
        "min_rr": number;
        "max_positions": number;
        "alt_min": number;
        "alt_max": number;
        "btceth_min": number;
        "btceth_max": number;
        "max_margin_pct": number;
        "alt_leverage"?: number;
        "btceth_leverage"?: number;
      };
      "account": {
        "total_equity": number;
        "available_balance": number;
        "total_pnl": number;
        "total_pnl_pct": number;
        "margin_used": number;
        "margin_used_pct": number;
        "position_count": number;
      };
      "positions": Array<{
        "symbol": string;
        "side": "LONG" | "SHORT";
        "entry_price": number;
        "mark_price": number;
        "quantity": number;
        "leverage": number;
        "unrealized_pnl": number;
        "unrealized_pnl_pct": number;
        "liquidation_price": number;
        "margin_used": number;
        "update_time_ms": number;
      }>;
      "market_data": {
        "[SYMBOL]": {
          "current_price": number;
          "price_change_1h": number;
          "price_change_4h": number;
          "macd": number;
          "rsi7": number;
          "rsi14": number;
        };
      };
      "candidate_coins": Array<{
        "symbol": string;
        "sources": string[];
      }>;
      "performance": {
        "sharpe_ratio": number;
      };
      "meta": {
        "time_utc_ms": number;
      };
    };
  }
}
```

**Response:**
```typescript
{
  "jsonrpc": "2.0",
  "id": <number>,
  "result": {
    "received": boolean;
    "queue_depth": number;
    "duplicate"?: boolean;
    "reason"?: string;
  }
}
```

**Features:**
- Zod schema validation
- Queue-based processing
- Deduplication support
- Partial payload support
- Derivatives enrichment
- Listener-based event callbacks

---

## 3. JSON Schema Format Used

### Schema Definition Location

Schemas are stored in `/Users/greg/Dev/git/workflow-manager/schemas/`

### Example: LLM Snapshot Schema Structure

**File:** `schemas/llm-snapshot.schema.json`

```json
{
  "type": "object",
  "properties": {
    "job_id": {
      "type": "string",
      "minLength": 1
    },
    "snapshot_id": {
      "type": "string",
      "minLength": 1
    },
    "snapshot": {
      "type": "object",
      "properties": {
        "risk_controls": {
          "type": "object",
          "properties": { /* ... */ },
          "required": ["min_rr", "max_positions", /* ... */],
          "additionalProperties": false
        },
        "account": {
          "type": "object",
          "properties": { /* ... */ },
          "required": ["total_equity", /* ... */],
          "additionalProperties": false
        }
        /* ... more properties ... */
      },
      "required": [
        "risk_controls",
        "account",
        "positions",
        "market_data",
        "candidate_coins",
        "performance",
        "meta"
      ],
      "additionalProperties": false
    }
  },
  "required": ["job_id", "snapshot_id", "deadline_ms", "snapshot"],
  "additionalProperties": false
}
```

### Key Schema Features

1. **Required Fields:** Specified in `required` array
2. **Type Validation:** Each property has a `type` field
3. **Constraints:** 
   - `minLength`, `maxLength` for strings
   - `minimum`, `exclusiveMinimum` for numbers
   - `pattern` for regex validation
4. **Nested Objects:** Full validation of nested structures
5. **Enums:** `enum` field for restricted values
6. **Additional Properties:** Set to `false` for strict validation

---

## 4. How Nodes Are Defined in Workflows

### Workflow Node Structure

Nodes are defined in the `nodes` section of a workflow YAML file.

**File:** `workflows/llm_trading_bot/event-driven-trading.yaml`

### Node Definition Format

```yaml
nodes:
  <node-id>:
    component_type: <type>           # Required: agent, llm, tool, mcp, script, flow, map
    type: <node-type>                # Node-specific type
    label: <display-label>           # Optional: Display name
    tool: <tool-name>                # Tool identifier (for tool nodes)
    params:                          # Tool-specific parameters
      <param-key>: <value>
    metadata:                        # Optional: Editor metadata
      position:
        x: <number>
        y: <number>
      color: <hex-color>
      notes: <string>
    inputs:                          # Optional: Input mappings
      <input-key>: <source-expression>
    outputs:                         # Optional: Output definitions
      - <output-name>
    outputSchema:                    # Optional: Output schema
      type: object
      properties: {}
```

### Example: RPC Server Node

**From:** `workflows/llm_trading_bot/event-driven-trading.yaml`

```yaml
nodes:
  RPC Server:
    component_type: tool
    type: rpc_server
    tool: rpc_server
    label: RPCd
    params:
      action: start
      host: 127.0.0.1
      port: 7878
      validate: true
      timeoutMs: 5000
      eventName: snapshot
      schemaPath: schemas/llm-snapshot.schema.json
      validateSchema: true
      allowPartial: false
      eventMaxAgeMs: 180000
      saveEvents: false
      environmentId: dummy/repo
      environmentLabel: dummy/repo
      gitRef: test
      bestOfN: 1
      enrichmentMode: backend
    metadata:
      position:
        x: 56
        y: -83
```

### Node Component Types

| Type | Purpose | Example |
|------|---------|---------|
| `agent` | Claude/LLM agent execution | AI decision making |
| `llm` | Direct LLM call | Model inference |
| `tool` | Tool execution (rpc_server, script_runner, etc.) | Data processing |
| `mcp` | Model Context Protocol tool | External tools |
| `script` | JavaScript execution | Data transformation |
| `flow` | Control flow (loops, branches) | Conditional logic |
| `map` | Parallel mapping | Batch processing |

---

## 5. RPC Client Structure

### Client Interfaces

**File:** `/Users/greg/Dev/git/workflow-manager/src/ingest/RPCClient.ts`

```typescript
export interface RPCClientOptions {
  host: string;
  port: number;
  method?: string;
  payload: any;
  timeout?: number;
}

export interface RPCClientResult {
  success: boolean;
  response?: any;
  error?: string;
  duration: number;
}

export async function sendRPCRequest(
  options: RPCClientOptions
): Promise<RPCClientResult>
```

### Client Usage Example

```typescript
import { sendRPCRequest } from './RPCClient';

const result = await sendRPCRequest({
  host: '127.0.0.1',
  port: 7878,
  method: 'ingest.push',
  payload: {
    channel: 'query_results',
    payload: {
      query: 'What are historical BTC patterns?',
      results: [...]
    }
  },
  timeout: 5000
});

if (result.success) {
  console.log('Request successful:', result.response);
} else {
  console.error('Request failed:', result.error);
}
```

### Server Startup Interface

```typescript
export interface StartOptions {
  host?: string;                    // Default: '127.0.0.1'
  port?: number;                    // Default: 7878
  secret?: string;                  // Optional: HMAC-SHA256 signature
  eventName?: string;               // Event variable name
  getState?: (key: string) => any;
  setState?: (key: string, value: any) => Promise<void>;
  workflowDir?: string;             // For schema path resolution
  schemaPath?: string;              // JSON schema for validation
  validateSchema?: boolean;         // Default: true
  mergeStrategy?: 'overwrite' | 'shallow' | 'deep' | 'append';  // Default: 'deep'
  allowPartial?: boolean;           // Default: false
  onIncomingData?: EventCallback;   // Event callback function
  eventMaxAgeMs?: number;           // Default: 180000 (3 min)
  saveEvents?: boolean;             // Default: false
}

export async function startRPCServer(
  options: StartOptions = {}
): Promise<ServerHandle>
```

---

## 6. How to Define a New Workflow Node

### Step 1: Create the JSON Schema (if needed)

**File:** `schemas/rag-query.schema.json`

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "minLength": 1,
      "description": "RAG query text"
    },
    "context": {
      "type": "object",
      "properties": {
        "market_data": { "type": "object" },
        "historical_patterns": { "type": "array" },
        "account_state": { "type": "object" }
      },
      "required": ["market_data"]
    },
    "max_results": {
      "type": "integer",
      "minimum": 1,
      "maximum": 100,
      "default": 10
    }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

### Step 2: Define Node in Workflow YAML

**File:** `workflows/your-workflow.yaml`

```yaml
nodes:
  RAG Query Service:
    component_type: tool
    type: rag_query
    tool: rag_query
    label: RAG Query
    params:
      action: start
      host: 127.0.0.1
      port: 8000
      schemaPath: schemas/rag-query.schema.json
      validateSchema: true
      eventName: rag_results
      allowPartial: false
      eventMaxAgeMs: 180000
      mergeStrategy: deep
    metadata:
      position:
        x: 200
        y: 100
      notes: "RAG query processing service"
    outputs:
      - results
      - metadata
```

### Step 3: Define Node Options (for UI)

**File:** `src/workflow/nodeOptionSpecs.ts`

```typescript
import { OptionDescriptor } from './nodeOptionSpecs';

export const ragQueryNodeOptions: OptionDescriptor[] = [
  {
    key: 'host',
    label: 'Host',
    path: 'params.host',
    required: true,
    group: 'connection',
    defaultValue: '127.0.0.1',
    input: {
      kind: 'text',
      placeholder: '127.0.0.1'
    }
  },
  {
    key: 'port',
    label: 'Port',
    path: 'params.port',
    required: true,
    group: 'connection',
    defaultValue: 8000,
    input: {
      kind: 'number',
      min: 1024,
      max: 65535,
      step: 1
    }
  },
  {
    key: 'schemaPath',
    label: 'Schema Path',
    path: 'params.schemaPath',
    group: 'validation',
    input: {
      kind: 'text',
      placeholder: 'schemas/rag-query.schema.json'
    }
  },
  {
    key: 'validateSchema',
    label: 'Validate Schema',
    path: 'params.validateSchema',
    group: 'validation',
    defaultValue: true,
    input: { kind: 'toggle' }
  },
  {
    key: 'eventName',
    label: 'Event Variable Name',
    path: 'params.eventName',
    group: 'state',
    defaultValue: 'rag_results',
    input: {
      kind: 'text',
      placeholder: 'rag_results'
    }
  },
  {
    key: 'mergeStrategy',
    label: 'Merge Strategy',
    path: 'params.mergeStrategy',
    group: 'data',
    defaultValue: 'deep',
    input: {
      kind: 'select',
      options: [
        { value: 'overwrite', label: 'Overwrite' },
        { value: 'shallow', label: 'Shallow Merge' },
        { value: 'deep', label: 'Deep Merge' },
        { value: 'append', label: 'Append' }
      ]
    }
  }
];
```

### Step 4: Register in Workflow Schema

**File:** `src/dag/schema.ts`

Add RAG query node definition to the unified node schema:

```typescript
const RagQueryNodeSchema = z.object({
  component_type: z.literal('tool'),
  type: z.literal('rag_query'),
  tool: z.literal('rag_query'),
  params: z.object({
    action: z.enum(['start', 'stop']).optional(),
    host: z.string().default('127.0.0.1'),
    port: z.number().min(1024).max(65535).default(8000),
    schemaPath: z.string().optional(),
    validateSchema: z.boolean().default(true),
    eventName: z.string().default('rag_results'),
    allowPartial: z.boolean().default(false),
    eventMaxAgeMs: z.number().positive().default(180000),
    mergeStrategy: z.enum(['overwrite', 'shallow', 'deep', 'append']).default('deep')
  }).passthrough(),
  metadata: NodeMetadataSchema
}).passthrough();
```

---

## 7. Implementing RAG Query as JSON-RPC Endpoint

### Implementation Strategy

Your RAG query service should expose a JSON-RPC 2.0 endpoint that:

1. **Listens on a configurable port** (default: 8000)
2. **Implements method handlers** for RAG queries
3. **Validates input against JSON schema**
4. **Returns standardized JSON-RPC responses**
5. **Integrates with workflow-manager state** via callbacks

### Minimal Implementation

```typescript
import { createServer, IncomingMessage, ServerResponse } from 'http';

export interface RAGQueryParams {
  query: string;
  context?: {
    market_data?: object;
    historical_patterns?: any[];
    account_state?: object;
  };
  max_results?: number;
}

export interface RAGQueryResult {
  query: string;
  results: Array<{
    content: string;
    relevance_score: number;
    source?: string;
  }>;
  metadata?: object;
}

export class RAGQueryServer {
  private server: any = null;
  
  async start(options: {
    host?: string;
    port?: number;
    schemaPath?: string;
    validateSchema?: boolean;
  } = {}) {
    const host = options.host ?? '127.0.0.1';
    const port = options.port ?? 8000;
    
    this.server = createServer(async (req, res) => {
      try {
        if (req.method === 'GET' && req.url === '/health') {
          return this.sendJson(res, 200, { ok: true });
        }
        
        if (req.method === 'POST' && req.url === '/query') {
          const body = await this.readBody(req);
          const parsed = JSON.parse(body);
          
          // Handle JSON-RPC 2.0
          if (parsed.jsonrpc === '2.0') {
            if (parsed.method === 'rag.query') {
              const result = await this.handleRAGQuery(parsed.params);
              return this.sendJson(res, 200, {
                jsonrpc: '2.0',
                id: parsed.id,
                result
              });
            } else {
              return this.sendJson(res, 400, {
                jsonrpc: '2.0',
                id: parsed.id,
                error: { code: -32601, message: 'Method not found' }
              });
            }
          }
          
          // Handle direct call
          const result = await this.handleRAGQuery(parsed);
          this.sendJson(res, 200, result);
        } else {
          this.sendJson(res, 404, { error: 'not_found' });
        }
      } catch (err: any) {
        this.sendJson(res, 500, {
          error: 'internal_error',
          message: err?.message ?? String(err)
        });
      }
    });
    
    await new Promise<void>((resolve) => {
      this.server.listen(port, host, resolve);
    });
    
    return { url: `http://${host}:${port}` };
  }
  
  private async handleRAGQuery(params: RAGQueryParams): Promise<RAGQueryResult> {
    // Your RAG implementation here
    return {
      query: params.query,
      results: [
        {
          content: 'RAG result based on query',
          relevance_score: 0.95,
          source: 'knowledge_base'
        }
      ],
      metadata: {
        processed_at: new Date().toISOString(),
        context_keys: Object.keys(params.context ?? {})
      }
    };
  }
  
  private async readBody(req: IncomingMessage): Promise<string> {
    return new Promise((resolve, reject) => {
      const chunks: Buffer[] = [];
      req.on('data', (c) => chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c)));
      req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
      req.on('error', reject);
    });
  }
  
  private sendJson(res: ServerResponse, code: number, data: any) {
    const payload = JSON.stringify(data);
    res.writeHead(code, {
      'content-type': 'application/json',
      'content-length': Buffer.byteLength(payload)
    });
    res.end(payload);
  }
  
  async stop() {
    if (this.server) {
      await new Promise<void>((resolve) => {
        this.server.close(() => resolve());
      });
    }
  }
}
```

### Integration with Workflow

In your workflow, call the RAG query service via RPC:

```yaml
edges:
  - from: "RPC Server"
    to: "snapshot_preprocessor"
  - from: "snapshot_preprocessor"
    to: "RAG Query Service"
    data:
      - fromProperty: "market_snapshot"
        toProperty: "params.context.market_data"

nodes:
  RAG Query Service:
    component_type: tool
    type: rag_query
    tool: rag_query
    params:
      action: start
      host: 127.0.0.1
      port: 8000
      schemaPath: schemas/rag-query.schema.json
      eventName: rag_results
  
  RAG Query Call:
    component_type: script
    type: script
    language: javascript
    script: |
      const { sendRPCRequest } = require('./src/ingest/RPCClient');
      
      const result = await sendRPCRequest({
        host: '127.0.0.1',
        port: 8000,
        method: 'rag.query',
        payload: {
          query: inputs.market_snapshot,
          context: {
            market_data: inputs.market_snapshot,
            account_state: inputs.account_state,
            historical_patterns: inputs.historical_context
          },
          max_results: 10
        },
        timeout: 30000
      });
      
      return {
        rag_response: result.response?.result,
        query_status: result.success ? 'success' : 'failed'
      };
```

---

## Summary

| Concept | Location | Key Points |
|---------|----------|-----------|
| **RPC Protocol** | Standard JSON-RPC 2.0 | jsonrpc, id, method, params, result/error |
| **RPC Server** | `/src/ingest/RPCServer.ts` | Generic event ingestion, schema validation |
| **RPC Client** | `/src/ingest/RPCClient.ts` | sendRPCRequest() for calling RPC methods |
| **Snapshot Intake** | `/src/ingest/LlmSnapshotIntake.ts` | Specialized endpoint for trading data |
| **Schemas** | `/schemas/` directory | JSON Schema format for validation |
| **Node Definition** | Workflow YAML `nodes` section | component_type, type, params |
| **Node Options** | `/src/workflow/nodeOptionSpecs.ts` | UI descriptors for node configuration |
| **Schema Definition** | `/src/dag/schema.ts` | Zod schemas for validation |

---

## Next Steps for RAG Integration

1. **Create RAG schema:** `schemas/rag-query.schema.json`
2. **Implement RAG server:** Using the example above
3. **Define workflow node:** Add RAG node to your workflow YAML
4. **Register node options:** Add descriptors for UI
5. **Add to schema:** Update `src/dag/schema.ts`
6. **Test RPC calls:** Use curl or sendRPCRequest() to test

Example cURL request:
```bash
curl -X POST http://127.0.0.1:8000/query \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "rag.query",
    "params": {
      "query": "What are the support levels for BTC?",
      "max_results": 5
    }
  }'
```

