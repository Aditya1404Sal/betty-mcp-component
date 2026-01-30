use crate::types::*;
use crate::config;
use crate::actions;
use serde_json::{json, Value};

/// Process an MCP JSON-RPC request
pub fn process_rpc(server_id: &str, body: &str) -> Result<String, String> {
    // Parse JSON-RPC request
    let request: JsonRpcRequest = serde_json::from_str(body)
        .map_err(|e| format!("Invalid JSON-RPC request: {}", e))?;

    // Validate JSON-RPC version
    if request.jsonrpc != "2.0" {
        return create_error_response(
            request.id,
            -32600,
            "Invalid Request: jsonrpc must be '2.0'",
        );
    }

    // Load server configuration
    let server_config = config::load_server_config(server_id)?;

    // Route based on method
    let result = match request.method.as_str() {
        "initialize" => handle_initialize(&request.params),
        "tools/list" => handle_list_tools(&server_config),
        "tools/call" => handle_call_tool(&request.params, &server_config),
        _ => {
            return create_error_response(
                request.id,
                -32601,
                &format!("Method not found: {}", request.method),
            );
        }
    };

    // Create response
    match result {
        Ok(result_value) => create_success_response(request.id, result_value),
        Err(e) => create_error_response(request.id, -32000, &e),
    }
}

fn handle_initialize(params: &Value) -> Result<Value, String> {
    let protocol_version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("2024-11-05");

    let capabilities = params
        .get("capabilities")
        .cloned()
        .unwrap_or(json!({}));

    Ok(json!({
        "protocolVersion": protocol_version,
        "capabilities": capabilities,
        "serverInfo": {
            "name": "betty-mcp-server",
            "version": "0.1.0"
        }
    }))
}

fn handle_list_tools(server_config: &McpServerConfig) -> Result<Value, String> {
    // Convert ToolWithAction to Tool (removing action-id from response)
    let tools: Vec<Tool> = server_config
        .tools
        .iter()
        .map(|t| t.tool.clone())
        .collect();

    let result = ListToolsResult {
        tools,
        next_cursor: None,
    };

    Ok(serde_json::to_value(result)
        .map_err(|e| format!("Failed to serialize tools list: {}", e))?)
}

fn handle_call_tool(params: &Value, server_config: &McpServerConfig) -> Result<Value, String> {
    // Parse tool call parameters
    let call_params: CallToolParams = serde_json::from_value(params.clone())
        .map_err(|e| format!("Invalid tool call parameters: {}", e))?;

    // Find the tool in the configuration
    let tool_with_action = server_config
        .tools
        .iter()
        .find(|t| t.tool.name == call_params.name)
        .ok_or_else(|| format!("Tool '{}' not found", call_params.name))?;

    // Validate arguments against input schema
    validate_arguments(&call_params.arguments, &tool_with_action.tool.input_schema)?;

    // Execute the mapped action
    let action_result = actions::execute_mapped_action(
        &tool_with_action.action_id,
        &call_params.arguments,
    )?;

    // Parse action output into MCP content
    let content = actions::parse_action_output(&action_result)?;

    // Create tool call result
    let result = CallToolResult {
        content,
        is_error: Some(false),
    };

    Ok(serde_json::to_value(result)
        .map_err(|e| format!("Failed to serialize tool result: {}", e))?)
}

fn validate_arguments(arguments: &Value, schema: &Value) -> Result<(), String> {
    // Basic validation: check required fields
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        let args_obj = arguments.as_object()
            .ok_or("Arguments must be an object")?;

        for req_field in required {
            let field_name = req_field.as_str()
                .ok_or("Required field name must be a string")?;
            
            if !args_obj.contains_key(field_name) {
                return Err(format!("Missing required argument: {}", field_name));
            }
        }
    }

    // Additional validation could be added here:
    // - Type checking against schema properties
    // - Enum validation
    // - Format validation
    // For now, we do basic required field checking

    Ok(())
}

fn create_success_response(id: Option<Value>, result: Value) -> Result<String, String> {
    let response = JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    };

    serde_json::to_string(&response)
        .map_err(|e| format!("Failed to serialize response: {}", e))
}

fn create_error_response(id: Option<Value>, code: i32, message: &str) -> Result<String, String> {
    let response = JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }),
    };

    serde_json::to_string(&response)
        .map_err(|e| format!("Failed to serialize error response: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_arguments() {
        let schema = json!({
            "type": "object",
            "properties": {
                "location": { "type": "string" },
                "unit": { "type": "string" }
            },
            "required": ["location"]
        });

        // Valid arguments
        let valid_args = json!({
            "location": "Amsterdam",
            "unit": "celsius"
        });
        assert!(validate_arguments(&valid_args, &schema).is_ok());

        // Missing required field
        let invalid_args = json!({
            "unit": "celsius"
        });
        assert!(validate_arguments(&invalid_args, &schema).is_err());
    }
}