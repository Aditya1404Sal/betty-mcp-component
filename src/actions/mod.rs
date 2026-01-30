use crate::types::*;
use serde_json::{json, Value};

/// Execute a wasmCloud action by its ID
pub fn execute_mapped_action(action_id: &str, arguments: &Value) -> Result<ActionResponse, String> {
    // TODO: Replace with actual betty:actions/executor WIT import when available
    // For now, use a mock implementation
    
    // This would eventually call the WIT-imported function:
    // use crate::betty::actions::executor;
    // let payload = ActionPayload {
    //     action_id: action_id.to_string(),
    //     arguments: arguments.clone(),
    // };
    // let result = executor::perform_action(&serde_json::to_string(&payload).unwrap());
    
    mock_action_execution(action_id, arguments)
}

/// Parse action response into MCP content blocks
pub fn parse_action_output(action_response: &ActionResponse) -> Result<Vec<ContentBlock>, String> {
    if !action_response.success {
        let error_msg = action_response.error.as_deref().unwrap_or("Unknown error");
        return Ok(vec![ContentBlock::Text {
            text: format!("Error: {}", error_msg),
        }]);
    }

    // Parse the action data into content blocks
    match &action_response.data {
        Some(data) => {
            // If the data is a string, return it as text
            if let Some(text) = data.as_str() {
                Ok(vec![ContentBlock::Text {
                    text: text.to_string(),
                }])
            }
            // If the data is an object with a "text" field, use that
            else if let Some(text) = data.get("text").and_then(|t| t.as_str()) {
                Ok(vec![ContentBlock::Text {
                    text: text.to_string(),
                }])
            }
            // If the data is an object with a "content" array, use that
            else if let Some(content_array) = data.get("content").and_then(|c| c.as_array()) {
                parse_content_array(content_array)
            }
            // Otherwise, serialize the entire data as JSON text
            else {
                Ok(vec![ContentBlock::Text {
                    text: serde_json::to_string_pretty(data)
                        .unwrap_or_else(|_| data.to_string()),
                }])
            }
        }
        None => Ok(vec![ContentBlock::Text {
            text: "Action completed successfully".to_string(),
        }]),
    }
}

fn parse_content_array(content_array: &[Value]) -> Result<Vec<ContentBlock>, String> {
    let mut blocks = Vec::new();
    
    for item in content_array {
        if let Some(content_type) = item.get("type").and_then(|t| t.as_str()) {
            match content_type {
                "text" => {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        blocks.push(ContentBlock::Text {
                            text: text.to_string(),
                        });
                    }
                }
                "image" => {
                    if let (Some(data), Some(mime_type)) = (
                        item.get("data").and_then(|d| d.as_str()),
                        item.get("mimeType").and_then(|m| m.as_str()),
                    ) {
                        blocks.push(ContentBlock::Image {
                            data: data.to_string(),
                            mime_type: mime_type.to_string(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    
    Ok(blocks)
}

// Mock action execution for testing
fn mock_action_execution(action_id: &str, arguments: &Value) -> Result<ActionResponse, String> {
    match action_id {
        "action-weather-get" => {
            let location = arguments.get("location")
                .and_then(|l| l.as_str())
                .ok_or("Missing location")?;
            
            let unit = arguments.get("unit")
                .and_then(|u| u.as_str())
                .unwrap_or("celsius");
            
            let temp = if unit == "fahrenheit" { 72 } else { 22 };
            
            Ok(ActionResponse {
                success: true,
                data: Some(json!({
                    "text": format!(
                        "Current weather in {}: {}°{}, partly cloudy with light winds",
                        location,
                        temp,
                        if unit == "fahrenheit" { "F" } else { "C" }
                    )
                })),
                error: None,
            })
        }
        "action-calc-add" => {
            let a = arguments.get("a")
                .and_then(|v| v.as_f64())
                .ok_or("Missing or invalid parameter 'a'")?;
            
            let b = arguments.get("b")
                .and_then(|v| v.as_f64())
                .ok_or("Missing or invalid parameter 'b'")?;
            
            let result = a + b;
            
            Ok(ActionResponse {
                success: true,
                data: Some(json!({
                    "text": format!("{} + {} = {}", a, b, result)
                })),
                error: None,
            })
        }
        _ => {
            Err(format!("Action '{}' not implemented", action_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_weather_action() {
        let args = json!({
            "location": "Amsterdam",
            "unit": "celsius"
        });
        
        let result = mock_action_execution("action-weather-get", &args).unwrap();
        assert!(result.success);
        assert!(result.data.is_some());
    }

    #[test]
    fn test_mock_calculator_action() {
        let args = json!({
            "a": 5,
            "b": 3
        });
        
        let result = mock_action_execution("action-calc-add", &args).unwrap();
        assert!(result.success);
    }

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
}