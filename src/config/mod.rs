use crate::types::{McpServerConfig, McpServersConfig};
use crate::wasi::config::store::get_all;

/// Check if a server ID exists in the configuration
pub fn check_server_id_exists(server_id: &str) -> Result<(), String> {
    eprintln!(
        "Checking if server ID '{}' exists in configuration",
        server_id
    );

    match get_all() {
        Ok(config) => {
            eprintln!("Runtime configuration keys available:");
            for (key, value) in config.iter() {
                eprintln!("Config key: {} = {}", key, value);
            }

            if config.is_empty() {
                eprintln!("No runtime configuration keys found");
            }

            // Try to load the servers config and check if server_id exists
            let servers_config = load_all_servers_config_from_runtime(&config)?;

            // Check if the server ID exists
            let server_exists = servers_config
                .mcp_servers
                .iter()
                .any(|server| server.id == server_id);

            if server_exists {
                eprintln!("Server ID '{}' found in configuration", server_id);
                Ok(())
            } else {
                Err(format!(
                    "MCP server '{}' not found in configuration",
                    server_id
                ))
            }
        }
        Err(e) => {
            eprintln!("Failed to retrieve runtime configuration: {:?}", e);
            Err(format!("Failed to retrieve runtime configuration: {:?}", e))
        }
    }
}

/// Load server configuration for a specific MCP server ID
pub fn load_server_config(server_id: &str) -> Result<McpServerConfig, String> {
    // Load all MCP servers configuration
    let config = load_all_servers_config()?;

    // Find the server with matching ID
    config
        .mcp_servers
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| format!("MCP server '{}' not found in configuration", server_id))
}

/// Load all MCP servers from configuration
fn load_all_servers_config() -> Result<McpServersConfig, String> {
    // Try to load from wasi:config first
    match load_from_wasi_config() {
        Some(config) => Ok(config),
        None => {
            // Fallback to hardcoded configuration for testing
            load_default_config()
        }
    }
}

fn load_from_wasi_config() -> Option<McpServersConfig> {
    match get_all() {
        Ok(config) => load_all_servers_config_from_runtime(&config).ok(),
        Err(e) => {
            eprintln!("Failed to get wasi config: {:?}", e);
            None
        }
    }
}

fn load_all_servers_config_from_runtime(
    config: &Vec<(String, String)>,
) -> Result<McpServersConfig, String> {
    // Look for "mcp_servers" key
    for (key, value) in config {
        if key == "mcp_servers" {
            return serde_json::from_str(value)
                .map_err(|e| format!("Failed to parse mcp_servers config: {}", e));
        }
    }
    Err("mcp_servers key not found in runtime configuration".to_string())
}

fn load_default_config() -> Result<McpServersConfig, String> {
    // Default configuration for testing
    let config_json = r#"{
        "mcp-servers": [
            {
                "id": "weather-server-001",
                "tools": [
                    {
                        "action-id": "action-weather-get",
                        "name": "get_weather",
                        "description": "Haalt de huidige weersomstandigheden op voor een specifieke locatie.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "location": {
                                    "type": "string",
                                    "description": "De stad en provincie, bijv. Amsterdam, NH"
                                },
                                "unit": {
                                    "type": "string",
                                    "enum": ["celsius", "fahrenheit"],
                                    "description": "De temperatuureenheid die gebruikt moet worden."
                                }
                            },
                            "required": ["location"]
                        }
                    }
                ]
            },
            {
                "id": "calculator-server-001",
                "tools": [
                    {
                        "action-id": "action-calc-add",
                        "name": "add_numbers",
                        "description": "Adds two numbers together",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "a": {
                                    "type": "number",
                                    "description": "First number"
                                },
                                "b": {
                                    "type": "number",
                                    "description": "Second number"
                                }
                            },
                            "required": ["a", "b"]
                        }
                    }
                ]
            }
        ]
    }"#;

    serde_json::from_str(config_json)
        .map_err(|e| format!("Failed to parse default configuration: {}", e))
}

// Requires wasm emv for testing, keeping it on-hold for now

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_load_default_config() -> Result<(), String> {
//         let config = load_all_servers_config()?;
//         assert_eq!(config.mcp_servers.len(), 2);
//         assert_eq!(config.mcp_servers[0].id, "weather-server-001");
//         assert_eq!(config.mcp_servers[1].id, "calculator-server-001");
//         Ok(())
//     }

//     #[test]
//     fn test_load_server_config() {
//         let config = load_server_config("weather-server-001").unwrap();
//         assert_eq!(config.id, "weather-server-001");
//         assert_eq!(config.tools.len(), 1);
//         assert_eq!(config.tools[0].tool.name, "get_weather");
//     }

//     #[test]
//     fn test_load_nonexistent_server() {
//         let result = load_server_config("nonexistent-server");
//         assert!(result.is_err());
//     }
// }
