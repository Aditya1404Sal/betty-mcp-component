# HTTP-MCP Provider - Project Documentation

## 1. Overview

The **HTTP-MCP Provider** is a specialized wasmCloud component that bridges Model Context Protocol (MCP) clients with internal wasmCloud actions, providing a direct, high-performance ingress route.

### Key Features

- **Direct Routing**: Accessible via `app-name.betty.app/api/mcp/{mcp-server-id}`
- **Dynamic Tools**: Tool definitions loaded from WASI-Config
- **Secure Authentication**: API token validation via WASI-Secrets
- **Protocol Translation**: JSON-RPC 2.0 server translating MCP calls to actions

### Architecture Principles

1. Single entry point through wasmCloud HTTP ingress
2. Path-based routing to isolate MCP traffic (`/mcp/*`)
3. Configuration-driven tool-to-action mapping
4. Stateless request handling for horizontal scalability

---

## 2. Technology Stack

### Dependencies

```toml
[dependencies]
wit-bindgen = "0.34"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### WIT Interfaces

- `wasi:http/incoming-handler@0.2.2` - HTTP request handling
- `wasi:config/runtime@0.2.0-draft` - Configuration management (future)
- `wasi:secrets/store@0.2.0-draft` - Secret storage (future)
- `betty:actions/executor` - Action execution (custom)

---

## 3. Detailed Workflows

### 3.1 Request Entry Flow

This is the top-level flow showing how every request is processed from entry to completion.

```
sequenceDiagram
    autonumber
    participant Client
    participant Ingress as wasmCloud Ingress
    participant Component as http-mcp Component
    participant Lib as lib.rs::handle_request()
    
    Client->>+Ingress: HTTP POST /api/mcp/{server-id}
    Note over Ingress: Path-based routing
    Ingress->>+Component: IncomingRequest
    Component->>+Lib: handle_request(req, resp_out)
    
    Lib->>Lib: validate_content_type()
    alt Content-Type invalid
        Lib-->>Component: 400 Bad Request
        Component-->>Ingress: Error Response
        Ingress-->>Client: HTTP 400
    end
    
    Lib->>Lib: extract_server_id()
    Lib->>Lib: extract_auth_token()
    Lib->>Lib: read_request_body()
    
    Note over Lib: Delegate to auth & mcp modules
    Lib-->>-Component: Result
    Component-->>-Ingress: Response
    Ingress-->>-Client: HTTP Response
```

**Description**: Every incoming HTTP request goes through validation (content-type, path parsing, authentication, body reading) before being routed to the appropriate handler.

---

### 3.2 Authentication Flow

This flow details the security validation process using API tokens.

```
sequenceDiagram
    autonumber
    participant Lib as lib.rs
    participant Auth as auth/mod.rs
    participant Secrets as wasi:secrets/store
    participant Env as Environment Variables
    
    Lib->>+Auth: is_authorized(token)
    
    Auth->>+Secrets: get("mcp_api_token")
    alt Secret exists
        Secrets-->>Auth: Secret value
        Auth->>Auth: constant_time_compare()
        Auth-->>Lib: true/false
    else Secret not found
        Secrets-->>-Auth: Error
        Auth->>+Env: get_env("MCP_API_TOKEN")
        alt Env var exists
            Env-->>Auth: Token value
            Auth->>Auth: constant_time_compare()
            Auth-->>Lib: true/false
        else No token configured
            Env-->>-Auth: None
            Note over Auth: Dev mode: allow access
            Auth-->>Lib: true (warning logged)
        end
    end
    
    Auth-->>-Lib: Authorized result
```

**Description**: Authentication attempts to retrieve the expected token from wasi:secrets first, then falls back to environment variables. Uses constant-time comparison to prevent timing attacks.

**Security Notes**:
- Constant-time comparison prevents timing attacks
- Multiple token sources for flexibility (secrets > env vars)
- Development mode allows testing without token configuration

---

### 3.3 Configuration Loading Flow

This flow shows how MCP server configurations are loaded and cached.

```
sequenceDiagram
    autonumber
    participant Router as mcp/router.rs
    participant Config as config/mod.rs
    participant WasiConfig as wasi:config/runtime
    participant Default as Default Config
    
    Router->>+Config: load_server_config(server_id)
    Config->>Config: load_all_servers_config()
    
    Config->>+WasiConfig: get("mcp_servers_config")
    alt Config exists
        WasiConfig-->>Config: JSON bytes
        Config->>Config: parse JSON
        Config->>Config: find_server(server_id)
        Config-->>Router: McpServerConfig
    else Config not found
        WasiConfig-->>-Config: Error
        Config->>+Default: load_default_config()
        Note over Default: Hardcoded test config
        Default-->>-Config: McpServersConfig
        Config->>Config: find_server(server_id)
        alt Server found
            Config-->>Router: McpServerConfig
        else Server not found
            Config-->>Router: Error: Server not found
        end
    end
    
    Config-->>-Router: Result
```

**Description**: Attempts to load configuration from WASI-Config, falling back to hardcoded defaults for development. Each server has a unique GUID and list of tools.

**Configuration Structure**:
```json
{
  "mcp-servers": [
    {
      "id": "weather-server-001",
      "tools": [
        {
          "action-id": "action-weather-get",
          "name": "get_weather",
          "description": "...",
          "inputSchema": {...}
        }
      ]
    }
  ]
}
```

---

### 3.4 JSON-RPC Routing Flow

This flow shows how JSON-RPC 2.0 requests are validated and routed to the correct handler.

```
sequenceDiagram
    autonumber
    participant Lib as lib.rs
    participant Router as mcp/router.rs
    participant Config as config/mod.rs
    
    Lib->>+Router: process_rpc(server_id, body)
    
    Router->>Router: parse JSON-RPC request
    alt Invalid JSON
        Router-->>Lib: Error: Invalid JSON
    end
    
    Router->>Router: validate jsonrpc == "2.0"
    alt Invalid version
        Router-->>Lib: Error -32600: Invalid Request
    end
    
    Router->>+Config: load_server_config(server_id)
    alt Server not found
        Config-->>Router: Error
        Router-->>Lib: Error response
    else Server found
        Config-->>-Router: McpServerConfig
    end
    
    Note over Router: Route based on method
    alt method == "initialize"
        Router->>Router: handle_initialize()
    else method == "tools/list"
        Router->>Router: handle_list_tools()
    else method == "tools/call"
        Router->>Router: handle_call_tool()
    else Unknown method
        Router-->>Lib: Error -32601: Method not found
    end
    
    Router->>Router: create_success_response()
    Router-->>-Lib: JSON-RPC response string
```

**Description**: Parses incoming JSON-RPC requests, validates protocol version, loads configuration, and routes to the appropriate method handler.

**Supported Methods**:
1. `initialize` - Protocol handshake
2. `tools/list` - Enumerate available tools
3. `tools/call` - Execute a specific tool

---

### 3.5 Tools List Flow (tools/list)

This flow allows clients to discover all available tools for a specific MCP server.

```
sequenceDiagram
    autonumber
    participant Client
    participant Router as mcp/router.rs
    participant Config as McpServerConfig
    
    Note over Client: Request: tools/list
    Client->>+Router: process_rpc("weather-server-001", body)
    
    Router->>Router: parse JSON-RPC request
    Router->>Router: validate method == "tools/list"
    
    Router->>+Config: access server_config.tools
    Config-->>-Router: Vec<ToolWithAction>
    
    Router->>Router: transform to Vec<Tool>
    Note over Router: Strip action-id from response
    
    Router->>Router: create ListToolsResult
    Note over Router: {<br/>  tools: [...],<br/>  nextCursor: null<br/>}
    
    Router->>Router: serialize to JSON
    Router->>Router: create_success_response()
    
    Router-->>-Client: JSON-RPC response
    Note over Client: Receives tool list<br/>with schemas
```

**Description**: Returns a list of all available tools for the specified MCP server, including their input schemas. The `action-id` field is stripped from the response to keep internal implementation details private.

**Example Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "get_weather",
        "description": "Get current weather conditions",
        "inputSchema": {
          "type": "object",
          "properties": {
            "location": {"type": "string"},
            "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
          },
          "required": ["location"]
        }
      }
    ]
  }
}
```

---

### 3.6 Tool Call Execution Flow (tools/call)

This is the most complex flow, executing wasmCloud actions based on MCP tool calls.

```
sequenceDiagram
    autonumber
    participant Client
    participant Router as mcp/router.rs
    participant Config as McpServerConfig
    participant Actions as actions/mod.rs
    participant Executor as betty:actions/executor
    
    Note over Client: Request: tools/call
    Client->>+Router: process_rpc(server_id, body)
    
    Router->>Router: parse CallToolParams
    Note over Router: Extract tool name & arguments
    
    Router->>+Config: find tool by name
    alt Tool not found
        Config-->>Router: None
        Router-->>Client: Error: Tool not found
    else Tool found
        Config-->>-Router: ToolWithAction
    end
    
    Router->>Router: validate_arguments(args, schema)
    Note over Router: Check required fields<br/>against inputSchema
    alt Validation fails
        Router-->>Client: Error: Missing required field
    end
    
    Router->>+Actions: execute_mapped_action(action_id, args)
    
    Actions->>Actions: prepare ActionPayload
    Actions->>+Executor: perform_action(payload)
    Note over Executor: Execute actual<br/>wasmCloud action
    Executor-->>-Actions: ActionResponse
    
    alt Action failed
        Actions->>Actions: parse error
        Actions-->>Router: ContentBlock::Text (error)
    else Action succeeded
        Actions->>Actions: parse_action_output()
        Note over Actions: Convert to MCP<br/>ContentBlock format
        Actions-->>-Router: Vec<ContentBlock>
    end
    
    Router->>Router: create CallToolResult
    Note over Router: {<br/>  content: [...],<br/>  isError: false<br/>}
    
    Router->>Router: create_success_response()
    Router-->>-Client: JSON-RPC response
```

**Description**: Validates tool call arguments against the schema, executes the mapped wasmCloud action, and translates the response into MCP content blocks.

**Validation Steps**:
1. Tool exists in configuration
2. Required arguments are present
3. Argument types match schema (basic validation)

**Action Execution**:
1. Map tool name to action ID
2. Prepare action payload
3. Execute via `betty:actions/executor`
4. Parse response into MCP format

---

### 3.7 Action Output Parsing Flow

This subflow shows how action responses are converted to MCP content blocks.

```
sequenceDiagram
    autonumber
    participant Actions as actions/mod.rs
    participant Response as ActionResponse
    participant ContentBlock as ContentBlock (enum)
    
    Actions->>+Response: parse_action_output(response)
    
    alt response.success == false
        Response->>Response: extract error message
        Response->>+ContentBlock: Text { error message }
        ContentBlock-->>-Response: ContentBlock
        Response-->>Actions: Vec<ContentBlock>
    end
    
    alt response.data is string
        Response->>+ContentBlock: Text { string }
        ContentBlock-->>-Response: ContentBlock
    else response.data has "text" field
        Response->>+ContentBlock: Text { data.text }
        ContentBlock-->>-Response: ContentBlock
    else response.data has "content" array
        loop For each content item
            alt item.type == "text"
                Response->>ContentBlock: Text { item.text }
            else item.type == "image"
                Response->>ContentBlock: Image { data, mimeType }
            else item.type == "resource"
                Response->>ContentBlock: Resource { uri, ... }
            end
        end
    else response.data is object
        Response->>Response: serialize to JSON
        Response->>+ContentBlock: Text { JSON string }
        ContentBlock-->>-Response: ContentBlock
    end
    
    Response-->>-Actions: Vec<ContentBlock>
```

**Description**: Intelligently parses action responses into MCP content blocks, supporting multiple output formats (text, objects, structured content arrays).

**Supported Output Formats**:
1. Plain string → Text block
2. Object with "text" field → Text block
3. Object with "content" array → Multiple typed blocks
4. Generic object → JSON serialized as text

---

### 3.8 Error Handling Flow

This flow shows how errors are handled at different layers.

```
sequenceDiagram
    autonumber
    participant Component as Component::mcp_handle
    participant Lib as lib.rs
    participant Handler as Any Handler
    participant Response as response_out
    
    Component->>+Lib: handle_request()
    
    Lib->>+Handler: process request
    alt Handler error
        Handler-->>-Lib: Err(message)
        Lib->>Lib: create error response
        Note over Lib: Map to appropriate<br/>HTTP status code
    end
    
    alt HTTP layer error
        Lib->>Lib: log error
        Lib->>+Response: send_error_response(status, body)
        Note over Response: HTTP error response
        Response-->>-Lib: Result
    end
    
    alt JSON-RPC error
        Lib->>Lib: create JSON-RPC error
        Note over Lib: {<br/>  jsonrpc: "2.0",<br/>  error: {code, message},<br/>  id: request_id<br/>}
        Lib->>+Response: send_success_response(json)
        Note over Response: HTTP 200 with<br/>JSON-RPC error
        Response-->>-Lib: Result
    end
    
    Lib-->>-Component: Result
    alt Top-level error
        Component->>Component: log error
        Note over Component: eprintln! to stderr
    end
```

**Description**: Multi-layered error handling distinguishing between HTTP-level errors (401, 400) and JSON-RPC protocol errors (method not found, invalid params).

**Error Categories**:
1. **HTTP Errors**: Authentication (401), Invalid format (400), Server error (500)
2. **JSON-RPC Errors**: Invalid request (-32600), Method not found (-32601), Internal error (-32000)
3. **Application Errors**: Tool not found, Argument validation, Action execution failures

---

## 4. File Structure

```
http-mcp-provider/
├── Cargo.toml
├── wit/
│   └── world.wit              # WIT interface definitions
└── src/
    ├── lib.rs                 # Entry point & HTTP handling
    ├── types.rs               # All type definitions
    ├── auth/
    │   └── mod.rs             # Authentication logic
    ├── config/
    │   └── mod.rs             # Configuration loading
    ├── mcp/
    │   ├── mod.rs             # Module exports
    │   └── router.rs          # JSON-RPC routing
    └── actions/
        └── mod.rs             # Action execution bridge
```

### Module Responsibilities

| Module | Responsibility | Key Functions |
|--------|---------------|---------------|
| `lib.rs` | Entry point, HTTP handling | `handle_request()`, `validate_content_type()`, `extract_server_id()` |
| `types.rs` | Type definitions | All structs and enums |
| `auth/mod.rs` | Security validation | `is_authorized()`, `constant_time_compare()` |
| `config/mod.rs` | Configuration management | `load_server_config()`, `load_all_servers_config()` |
| `mcp/router.rs` | Protocol handling | `process_rpc()`, `handle_list_tools()`, `handle_call_tool()` |
| `actions/mod.rs` | Action execution | `execute_mapped_action()`, `parse_action_output()` |

---

## 5. Data Structures

### Core Types

```rust
// JSON-RPC 2.0
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

// MCP Tool Definition
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

// Extended with action mapping
pub struct ToolWithAction {
    #[serde(flatten)]
    pub tool: Tool,
    pub action_id: String,
}

// Server Configuration
pub struct McpServerConfig {
    pub id: String,
    pub tools: Vec<ToolWithAction>,
}

// Tool Execution
pub struct CallToolParams {
    pub name: String,
    pub arguments: Value,
}

pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: Option<bool>,
}

// Action Bridge
pub struct ActionPayload {
    pub action_id: String,
    pub arguments: Value,
}

pub struct ActionResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}
```

---

## 6. Testing Strategy

### Unit Tests

Each module includes unit tests:

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test auth::tests
cargo test config::tests
cargo test mcp::router::tests
cargo test actions::tests
```

### Integration Testing

1. **Mock Action Testing**: Use mock actions in `actions/mod.rs`
2. **End-to-End Testing**: Full JSON-RPC request/response cycles
3. **Configuration Testing**: Validate config parsing and server lookup

### Test Scenarios

| Scenario | Module | Expected Result |
|----------|--------|-----------------|
| Valid authentication | `auth` | Returns `true` |
| Invalid token | `auth` | Returns `false` |
| Server config lookup | `config` | Returns `McpServerConfig` |
| Non-existent server | `config` | Returns error |
| tools/list request | `router` | Returns tool list |
| tools/call with valid args | `router` | Executes action |
| tools/call missing required arg | `router` | Returns validation error |
| Action execution success | `actions` | Returns content blocks |
| Action execution failure | `actions` | Returns error content |

---

## 7. Deployment Configuration

### wasmCloud Workload Setup

```yaml
apiVersion: wasmcloud.dev/v1alpha1
kind: WadmWorkload
metadata:
  name: http-mcp-provider
spec:
  components:
    - name: http-mcp
      image: ghcr.io/your-org/http-mcp-provider:latest
      traits:
        - type: spreadscaler
          properties:
            replicas: 3
        - type: link
          properties:
            target: httpserver
            namespace: wasi
            package: http
            interfaces:
              - incoming-handler
            source_config:
              - name: default-http
                properties:
                  address: "0.0.0.0:8080"
                  
  # Path-based routing configuration
  routing:
    - path: /mcp/*
      component: http-mcp
    - path: /*
      deny: true  # Deny all other paths
```

### ConfigMap Setup

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: mcp-servers-config
data:
  config.json: |
    {
      "mcp-servers": [
        {
          "id": "weather-server-001",
          "tools": [
            {
              "action-id": "action-weather-get",
              "name": "get_weather",
              "description": "Get current weather",
              "inputSchema": {
                "type": "object",
                "properties": {
                  "location": {"type": "string"},
                  "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                },
                "required": ["location"]
              }
            }
          ]
        }
      ]
    }
```

### Secret Setup

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: mcp-api-token
type: Opaque
stringData:
  token: "your-secure-api-token-here"
```

---

## 8. Development Roadmap

### Phase 1: Core Implementation ✓
- [x] Basic HTTP request handling
- [x] JSON-RPC 2.0 parsing and routing
- [x] Configuration loading (with defaults)
- [x] Authentication (with fallback)
- [x] Mock action execution
- [x] Tool listing and execution

### Phase 2: Integration
- [ ] WASI-Config integration
- [ ] WASI-Secrets integration
- [ ] Real `betty:actions/executor` WIT binding
- [ ] Comprehensive error handling
- [ ] Logging and observability

### Phase 3: Production Readiness
- [ ] Performance optimization
- [ ] Load testing
- [ ] Security audit
- [ ] Documentation
- [ ] Monitoring and alerting

---

## 9. API Reference

### Endpoints

#### Initialize
```json
// Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {}
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {"tools": {}},
    "serverInfo": {
      "name": "betty-mcp-server",
      "version": "0.1.0"
    }
  }
}
```

#### List Tools
```json
// Request
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}

// Response
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "get_weather",
        "description": "Get current weather conditions",
        "inputSchema": { /* JSON Schema */ }
      }
    ]
  }
}
```

#### Call Tool
```json
// Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "get_weather",
    "arguments": {
      "location": "Amsterdam",
      "unit": "celsius"
    }
  }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Current weather in Amsterdam: 22°C, partly cloudy"
      }
    ],
    "isError": false
  }
}
```

---

## 10. Security Considerations

1. **Authentication**: All requests require valid API token
2. **Path Isolation**: Only `/mcp/*` paths accepted
3. **Input Validation**: Arguments validated against JSON schemas
4. **Timing Attack Prevention**: Constant-time token comparison
5. **Error Handling**: No sensitive information in error messages

---

## 11. Performance Considerations

1. **Stateless Design**: Enables horizontal scaling
2. **Configuration Caching**: Config loaded once per request
3. **Minimal Dependencies**: Small wasm binary size
4. **Streaming Response**: Efficient HTTP response handling

---

## 12. Future Enhancements

1. **Rate Limiting**: Per-client request throttling
2. **Caching**: Tool list caching with TTL
3. **Metrics**: Prometheus-compatible metrics export
4. **Tracing**: Distributed tracing support
5. **Schema Validation**: Full JSON Schema validation
6. **WebSocket Support**: Long-lived MCP connections
7. **Multi-tenancy**: Per-tenant MCP server isolation