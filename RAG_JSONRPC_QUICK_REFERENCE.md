# RAG JSON-RPC Quick Reference

## Quick Start

### 1. RAG Server Implementation

```typescript
import { createServer } from 'http';

class RAGQueryServer {
  async start(options = {}) {
    const host = options.host ?? '127.0.0.1';
    const port = options.port ?? 8000;

    this.server = createServer(async (req, res) => {
      if (req.method === 'GET' && req.url === '/health') {
        return res.writeHead(200), res.end(JSON.stringify({ ok: true }));
      }

      if (req.method === 'POST' && req.url === '/query') {
        const body = await this.readBody(req);
        const msg = JSON.parse(body);

        // Handle JSON-RPC 2.0
        if (msg.jsonrpc === '2.0') {
          const result = await this.handleQuery(msg.params);
          return res.writeHead(200, { 'content-type': 'application/json' }),
            res.end(JSON.stringify({
              jsonrpc: '2.0',
              id: msg.id,
              result
            }));
        }

        // Handle direct call
        const result = await this.handleQuery(msg);
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify(result));
      }
    });

    return new Promise(resolve => {
      this.server.listen(port, host, () => resolve({ url: `http://${host}:${port}` }));
    });
  }

  private async handleQuery(params) {
    const { query, context = {}, max_results = 10 } = params;
    // Your RAG logic here
    return {
      query,
      results: [],
      metadata: { processed_at: new Date().toISOString() }
    };
  }

  private async readBody(req) {
    return new Promise((resolve, reject) => {
      const chunks = [];
      req.on('data', c => chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c)));
      req.on('end', () => resolve(Buffer.concat(chunks).toString()));
      req.on('error', reject);
    });
  }
}
```

### 2. Workflow Node Definition

Add to your `workflow.yaml`:

```yaml
nodes:
  RAG Query Service:
    component_type: tool
    type: rag_query
    tool: rag_query
    params:
      action: start
      host: 127.0.0.1
      port: 8000
      eventName: rag_results
      schemaPath: schemas/rag-query.schema.json
      validateSchema: true
    metadata:
      position: { x: 300, y: 150 }

  Call RAG:
    component_type: script
    language: javascript
    script: |
      const { sendRPCRequest } = require('./src/ingest/RPCClient');
      const result = await sendRPCRequest({
        host: '127.0.0.1',
        port: 8000,
        method: 'rag.query',
        payload: {
          query: inputs.market_snapshot,
          context: inputs,
          max_results: 10
        }
      });
      return { rag_results: result.response?.result };
```

### 3. JSON Schema

Create `schemas/rag-query.schema.json`:

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "minLength": 1
    },
    "context": {
      "type": "object"
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

---

## JSON-RPC Protocol

### Request
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "rag.query",
  "params": {
    "query": "What are the support levels?",
    "max_results": 5
  }
}
```

### Response (Success)
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "query": "What are the support levels?",
    "results": [
      {
        "content": "Support at 48000",
        "relevance_score": 0.95
      }
    ]
  }
}
```

### Response (Error)
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params"
  }
}
```

---

## Testing

### Health Check
```bash
curl http://127.0.0.1:8000/health
```

### Query via cURL
```bash
curl -X POST http://127.0.0.1:8000/query \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "rag.query",
    "params": {
      "query": "market support levels",
      "max_results": 5
    }
  }'
```

### Query via TypeScript
```typescript
import { sendRPCRequest } from './src/ingest/RPCClient';

const result = await sendRPCRequest({
  host: '127.0.0.1',
  port: 8000,
  method: 'rag.query',
  payload: {
    query: 'BTC support levels',
    max_results: 10
  },
  timeout: 5000
});

console.log(result.response?.result);
```

---

## Error Codes

| Code | Meaning |
|------|---------|
| -32601 | Method not found |
| -32602 | Invalid params |
| -32700 | Parse error |
| -32600 | Invalid request |
| -32603 | Internal error |

---

## Configuration Options

```typescript
{
  host: '127.0.0.1',              // Server host
  port: 8000,                     // Server port
  schemaPath: 'schemas/rag-query.schema.json',  // JSON schema file
  validateSchema: true,           // Validate against schema
  eventName: 'rag_results',       // State variable name
  allowPartial: false,            // Allow partial payloads
  eventMaxAgeMs: 180000,          // Max age in ms
  mergeStrategy: 'deep'           // deep|shallow|append|overwrite
}
```

---

## Common Patterns

### Direct RPC Call
```typescript
const body = JSON.stringify({
  jsonrpc: '2.0',
  id: Date.now(),
  method: 'rag.query',
  params: { query: 'text' }
});

const response = await fetch('http://127.0.0.1:8000/query', {
  method: 'POST',
  body
});
```

### Using sendRPCRequest
```typescript
const result = await sendRPCRequest({
  host: '127.0.0.1',
  port: 8000,
  method: 'rag.query',
  payload: { query: 'text' }
});
```

### Workflow Integration
```yaml
edges:
  - from: preprocessor
    to: RAG Query Service
    data:
      - fromProperty: market_snapshot
        toProperty: params.context.market_data
```

---

## Files to Create/Modify

1. **Create:** `schemas/rag-query.schema.json`
2. **Create:** RAG server implementation (your-rag-server.ts)
3. **Modify:** Your workflow YAML (add RAG node)
4. **Optional:** `src/dag/schema.ts` (register node schema)
5. **Optional:** `src/workflow/nodeOptionSpecs.ts` (UI options)

---

## Minimal Working Example

### Server (3 min setup)
```typescript
import { createServer } from 'http';

const server = createServer(async (req, res) => {
  if (req.url === '/health') {
    res.writeHead(200);
    res.end('{"ok":true}');
    return;
  }
  
  const body = await new Promise(r => {
    const chunks = [];
    req.on('data', c => chunks.push(c));
    req.on('end', () => r(Buffer.concat(chunks).toString()));
  });

  const msg = JSON.parse(body);
  const result = { query: msg.params.query, results: [] };

  res.writeHead(200);
  res.end(JSON.stringify({
    jsonrpc: '2.0',
    id: msg.id,
    result
  }));
});

server.listen(8000, '127.0.0.1', () => console.log('RAG server ready'));
```

### Test It
```bash
curl -X POST http://127.0.0.1:8000/query \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"rag.query","params":{"query":"test"}}'
```

---

## Key Resources

- Full Guide: `WORKFLOW_MANAGER_RPC_GUIDE.md`
- Summary: `WORKFLOW_MANAGER_EXPLORATION_SUMMARY.md`
- RPC Client: `/workflow-manager/src/ingest/RPCClient.ts`
- RPC Server: `/workflow-manager/src/ingest/RPCServer.ts`
- Example: `/workflow-manager/workflows/llm_trading_bot/event-driven-trading.yaml`

