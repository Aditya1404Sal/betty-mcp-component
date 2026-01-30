use crate::types::*;
use rust_mcp_schema::ContentBlock;
use serde_json::{json, Value};

/// Execute a wasmCloud action by its ID
pub fn execute_mapped_action(action_id: &str, arguments: &Value) -> Result<ActionResponse, String> {
    // TODO: Replace with actual betty:actions/executor WIT import when available
    // For now, use httpbin to simulate action execution

    // This would eventually call the WIT-imported function:
    // use crate::betty::actions::executor;
    // let payload = ActionPayload {
    //     action_id: action_id.to_string(),
    //     arguments: arguments.clone(),
    // };
    // let result = executor::perform_action(&serde_json::to_string(&payload).unwrap());
    // We mock it for now
    temp_execute_via_httpbin(action_id, arguments)
}

/// Parse action response into MCP content blocks
pub fn parse_action_output(action_response: &ActionResponse) -> Result<Vec<ContentBlock>, String> {
    if !action_response.success {
        let error_msg = action_response.error.as_deref().unwrap_or("Unknown error");
        return Ok(vec![ContentBlock::text_content(format!(
            "Error: {}",
            error_msg
        ))]);
    }

    // Parse the action data into content blocks
    match &action_response.data {
        Some(data) => {
            // If the data is a string, return it as text
            if let Some(text) = data.as_str() {
                Ok(vec![ContentBlock::text_content(text.to_string())])
            }
            // If the data is an object with a "text" field, use that
            else if let Some(text) = data.get("text").and_then(|t| t.as_str()) {
                Ok(vec![ContentBlock::text_content(text.to_string())])
            }
            // If the data is an object with a "content" array, use that
            else if let Some(content_array) = data.get("content").and_then(|c| c.as_array()) {
                parse_content_array(content_array)
            }
            // Otherwise, serialize the entire data as JSON text
            else {
                Ok(vec![ContentBlock::text_content(
                    serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string()),
                )])
            }
        }
        None => Ok(vec![ContentBlock::text_content(
            "Action completed successfully".to_string(),
        )]),
    }
}

fn parse_content_array(content_array: &[Value]) -> Result<Vec<ContentBlock>, String> {
    let mut blocks = Vec::new();

    for item in content_array {
        if let Some(content_type) = item.get("type").and_then(|t| t.as_str()) {
            match content_type {
                "text" => {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        blocks.push(ContentBlock::text_content(text.to_string()));
                    }
                }
                "image" => {
                    if let (Some(data), Some(mime_type)) = (
                        item.get("data").and_then(|d| d.as_str()),
                        item.get("mimeType").and_then(|m| m.as_str()),
                    ) {
                        blocks.push(ContentBlock::image_content(
                            data.to_string(),
                            mime_type.to_string(),
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    Ok(blocks)
}

/// Execute action via httpbin (temporary mock using real HTTP)
fn temp_execute_via_httpbin(action_id: &str, arguments: &Value) -> Result<ActionResponse, String> {
    use std::time::Duration;
    use waki::Client;

    eprintln!("[ACTION] Executing action '{}' via httpbin", action_id);
    eprintln!(
        "[ACTION] Arguments: {}",
        serde_json::to_string_pretty(arguments).unwrap_or_default()
    );

    // Prepare the payload to send to httpbin
    let payload = json!({
        "action_id": action_id,
        "arguments": arguments
    });

    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| format!("Failed to serialize payload: {}", e))?;

    eprintln!("[ACTION] Sending request to httpbin.org/post");

    // Make HTTP request to httpbin
    let resp = Client::new()
        .post("https://httpbin.org/post")
        .connect_timeout(Duration::from_secs(10))
        .headers([("Content-Type", "application/json")])
        .body(payload_str.as_bytes())
        .send()
        .map_err(|e| format!("HTTP request failed: {:?}", e))?;

    eprintln!(
        "[ACTION] Received response with status: {}",
        resp.status_code()
    );

    // Check status code
    if resp.status_code() != 200 {
        return Err(format!(
            "HTTP request failed with status: {}",
            resp.status_code()
        ));
    }

    // Parse the response body
    let body = resp
        .body()
        .map_err(|e| format!("Failed to read response body: {:?}", e))?;

    let body_str = String::from_utf8(body.to_vec())
        .map_err(|e| format!("Response body is not valid UTF-8: {}", e))?;

    eprintln!("[ACTION] Response body: {}", body_str);

    // Parse httpbin response (it echoes back our JSON in the "json" field)
    let httpbin_response: Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

    // Extract our original payload from httpbin's response
    let echoed_data = httpbin_response
        .get("json")
        .ok_or("httpbin response missing 'json' field")?;

    // Create a success response with the echoed data
    // In real implementation, this would be the actual action result
    let result_text = format!(
        "Action '{}' executed successfully via httpbin. Echoed arguments: {}",
        action_id,
        serde_json::to_string_pretty(&echoed_data.get("arguments")).unwrap_or_default()
    );

    Ok(ActionResponse {
        success: true,
        data: Some(json!({
            "text": result_text,
            "echoed_payload": echoed_data
        })),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_output() {
        let response = ActionResponse {
            success: true,
            data: Some(json!({
                "text": "Hello, world!"
            })),
            error: None,
        };

        let content = parse_action_output(&response).unwrap();
        assert_eq!(content.len(), 1);
    }

    // #[test]
    // fn test_parse_error_output() {
    //     let response = ActionResponse {
    //         success: false,
    //         data: None,
    //         error: Some("Something went wrong".to_string()),
    //     };

    //     let content = parse_action_output(&response).unwrap();
    //     assert_eq!(content.len(), 1);
    //     match &content[0] {
    //         CallToolResult::text_content(TextContent::new( text_content)) => {
    //             assert!(text_content[0].text.contains("Error:"));
    //         }
    //         _ => panic!("Expected text content block"),
    //     }
    // }
}
