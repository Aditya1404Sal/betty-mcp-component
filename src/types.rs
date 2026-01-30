use serde::{Deserialize, Serialize};
use serde_json::Value;

// Re-export canonical MCP types from rust-mcp-schema (2024_11_05 schema)
pub use rust_mcp_schema::mcp_2025_11_25::{
    CallToolRequestParams as CallToolParams, CallToolResult, JsonrpcErrorResponse, JsonrpcRequest,
    JsonrpcResponse, ListToolsResult, Tool, ToolInputSchema, JSONRPC_VERSION,
};

// ============ MCP Tool Types ============

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolWithAction {
    #[serde(flatten)]
    pub tool: Tool,
    #[serde(rename = "action-id")]
    pub action_id: String,
}

// ============ MCP Server Configuration ============

#[derive(Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub tools: Vec<ToolWithAction>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpServersConfig {
    #[serde(rename = "mcp-servers")]
    pub mcp_servers: Vec<McpServerConfig>,
}

// ============ Action Execution Types ============
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ActionPayload {
    pub action_id: String,
    pub arguments: Value,
}

#[derive(Debug, Deserialize)]
pub struct ActionResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}
