use serde_json::json;

wit_bindgen::generate!({
    world: "mcp",
    generate_all,
});

use exports::wasmcloud::mcp::mcp_handler::Guest as McpHandler;

mod config;
mod mcp;
mod actions;
mod types;

use crate::betty_blocks::auth::jwt::validate_token;

struct Component;

impl McpHandler for Component {
    fn mcp_handle(
        request: crate::wasi::http::types::IncomingRequest,
        response_out: crate::wasi::http::types::ResponseOutparam,
    ) {
        handle_request(request, response_out);
    }
}

fn handle_request(
    request: crate::wasi::http::types::IncomingRequest,
    response_out: crate::wasi::http::types::ResponseOutparam,
) {
    // Step 1: Validate Content-Type
    if let Err(e) = validate_content_type(&request) {
        send_error_response(response_out, 400, e);
        return;
    }

    // Step 2: Extract server ID from path
    let server_id = match extract_server_id(&request) {
        Ok(id) => id,
        Err(e) => {
            send_error_response(response_out, 400, e);
            return;
        }
    };

    // Log runtime config
    log_runtime_config();

    // Step 3: Authenticate request
    let headers = request.headers().entries().into_iter().map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string())).collect::<Vec<_>>();
    if validate_token(&headers).is_err() {
        send_error_response(response_out, 401, "Unauthorized".to_string());
        return;
    }

    // Step 4: Read request body
    let body = match read_request_body(&request) {
        Ok(b) => b,
        Err(e) => {
            send_error_response(response_out, 400, e);
            return;
        }
    };

    // Step 5: Process MCP RPC request
    match mcp::router::process_rpc(&server_id, &body) {
        Ok(result) => send_success_response(response_out, result),
        Err(e) => {
            if e.contains("Invalid JSON-RPC") {
                send_error_response(response_out, 500, "Invalid JSON-RPC request".to_string());
            } else {
                send_error_response(response_out, 400, e);
            }
        }
    }
}

fn validate_content_type(request: &crate::wasi::http::types::IncomingRequest) -> Result<(), String> {
    let headers = request.headers();
    let entries = headers.entries();
    
    for (key, value) in entries {
        if key.to_lowercase() == "content-type" {
            let value_str = String::from_utf8_lossy(&value);
            if value_str.contains("application/json") {
                return Ok(());
            }
        }
    }
    
    Err("Content-Type must be application/json".to_string())
}

fn extract_server_id(request: &crate::wasi::http::types::IncomingRequest) -> Result<String, String> {
    let path_with_query = request.path_with_query().ok_or("No path found")?;
    
    // Expected format: /mcp/{server-id}
    let parts: Vec<&str> = path_with_query.split('/').collect();
    
    if parts.len() >= 3 && parts[1] == "mcp" {
        Ok(parts[2].to_string())
    } else {
        Err("Invalid path format. Expected /mcp/{server-id}".to_string())
    }
}
// should fetch the configuration into the handle_request, where the server_id will be checked, aborted if not present.
fn log_runtime_config() {
    eprintln!("Retrieving runtime configuration");
    match crate::wasi::config::store::get_all() {
        Ok(config) => {
            eprintln!("Runtime configuration keys available:");
            for (key, value) in config.iter() {
                let value_str = String::from_utf8_lossy(value.as_bytes());
                eprintln!("Config key: {} = {}", key, value_str);
            }
            if config.is_empty() {
                eprintln!("No runtime configuration keys found");
            }
        }
        Err(e) => {
            eprintln!("Failed to retrieve runtime configuration: {:?}", e);
        }
    }
}

fn read_request_body(request: &crate::wasi::http::types::IncomingRequest) -> Result<String, String> {
    let body_stream = request.consume().map_err(|_| "Failed to get body stream")?;
    let input_stream = body_stream.stream().map_err(|_| "Failed to get input stream")?;
    
    let mut buf = Vec::new();
    loop {
        match input_stream.blocking_read(1024 * 1024) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Err(_) => break,
        }
    }
    
    String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8 in body: {}", e))
}

fn send_success_response(
    response_out: crate::wasi::http::types::ResponseOutparam,
    body: String,
){
    use crate::wasi::http::types::{Fields, OutgoingBody, OutgoingResponse};

    let headers = Fields::new();
    let _ = headers.set(&"content-type".to_string(), &[b"application/json".to_vec()]);

    let response = OutgoingResponse::new(headers);
    if let Err(e) = response.set_status_code(200) {
        eprintln!("Failed to set status code: {:?}", e);
        return;
    }

    let response_body = match response.body() {
        Ok(rb) => rb,
        Err(e) => {
            eprintln!("Failed to get response body: {:?}", e);
            return;
        }
    };
    crate::wasi::http::types::ResponseOutparam::set(response_out, Ok(response));

    let output_stream = match response_body.write() {
        Ok(os) => os,
        Err(e) => {
            eprintln!("Failed to get output stream: {:?}", e);
            return;
        }
    };
    if let Err(e) = output_stream.blocking_write_and_flush(body.as_bytes()) {
        eprintln!("Failed to write response: {:?}", e);
        return;
    }

    drop(output_stream);
    if let Err(e) = OutgoingBody::finish(response_body, None) {
        eprintln!("Failed to finish body: {:?}", e);
    }
}

fn send_error_response(
    response_out: crate::wasi::http::types::ResponseOutparam,
    status: u16,
    message: String,
) {
    use crate::wasi::http::types::{Fields, OutgoingBody, OutgoingResponse};

    let error_body = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": -32000,
            "message": message
        },
        "id": null
    });

    let headers = Fields::new();
    let _ = headers.set(&"content-type".to_string(), &[b"application/json".to_vec()]);

    let response = OutgoingResponse::new(headers);
    if let Err(e) = response.set_status_code(status) {
        eprintln!("Failed to set status code: {:?}", e);
        return;
    }

    let response_body = match response.body() {
        Ok(rb) => rb,
        Err(e) => {
            eprintln!("Failed to get response body: {:?}", e);
            return;
        }
    };
    crate::wasi::http::types::ResponseOutparam::set(response_out, Ok(response));

    let output_stream = match response_body.write() {
        Ok(os) => os,
        Err(e) => {
            eprintln!("Failed to get output stream: {:?}", e);
            return;
        }
    };
    let body_str = serde_json::to_string(&error_body).unwrap();
    if let Err(e) = output_stream.blocking_write_and_flush(body_str.as_bytes()) {
        eprintln!("Failed to write response: {:?}", e);
        return;
    }

    drop(output_stream);
    if let Err(e) = OutgoingBody::finish(response_body, None) {
        eprintln!("Failed to finish body: {:?}", e);
    }
}

export!(Component);