use serde_json::{json, Value};
use std::path::Path;
use std::io::ErrorKind;
use reqwest::header::{ACCEPT, HeaderMap, HeaderName, HeaderValue};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};

pub async fn handle_mcp_command(server_command: &str, user_input: &str) -> Result<String, String> {
    if server_command.trim().is_empty() {
        return Err("No MCP server command configured. Add one in Settings first.".to_string());
    }

    let request = parse_user_command(user_input)?;
    if is_http_endpoint(server_command) {
        return handle_http_command(server_command.trim(), request).await;
    }

    let (program, args) = resolve_server_command(server_command)?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start MCP server: {e}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open MCP stdin.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open MCP stdout.".to_string())?;
    let mut stdout = BufReader::new(stdout);

    initialize(&mut stdin, &mut stdout).await?;

    let output = match request {
        ParsedMcpCommand::ToolsList => tools_list(&mut stdin, &mut stdout).await?,
        ParsedMcpCommand::ToolsCall { name, arguments } => {
            tools_call(&mut stdin, &mut stdout, &name, arguments).await?
        }
    };

    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(output)
}

enum ParsedMcpCommand {
    ToolsList,
    ToolsCall { name: String, arguments: Value },
}

fn parse_user_command(user_input: &str) -> Result<ParsedMcpCommand, String> {
    let trimmed = user_input.trim();
    if !trimmed.starts_with("/mcp") {
        return Err("MCP commands must start with /mcp.".to_string());
    }

    let rest = trimmed.trim_start_matches("/mcp").trim();
    if rest.eq_ignore_ascii_case("tools") || rest.eq_ignore_ascii_case("tools/list") {
        return Ok(ParsedMcpCommand::ToolsList);
    }

    if let Some(after_call) = rest.strip_prefix("call ") {
        let after_call = after_call.trim();
        let first_space = after_call.find(char::is_whitespace);
        let (name, args_str) = match first_space {
            Some(idx) => (&after_call[..idx], after_call[idx..].trim()),
            None => (after_call, "{}"),
        };

        if name.is_empty() {
            return Err("Usage: /mcp call <tool_name> {\"arg\":\"value\"}".to_string());
        }

        let arguments = if args_str.is_empty() {
            json!({})
        } else if let Some(path_arg) = parse_quoted_string_arg(args_str) {
            json!({ "path": path_arg })
        } else {
            serde_json::from_str(args_str)
                .map_err(|e| format!("Invalid JSON arguments for /mcp call: {e}"))?
        };

        return Ok(ParsedMcpCommand::ToolsCall {
            name: name.to_string(),
            arguments,
        });
    }

    Err("Supported MCP commands: `/mcp tools` and `/mcp call <tool> {json}`.".to_string())
}

fn parse_quoted_string_arg(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.len() < 2 {
        return None;
    }

    let quote = trimmed.chars().next()?;
    if (quote != '"' && quote != '\'') || !trimmed.ends_with(quote) {
        return None;
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    if quote == '"' {
        Some(inner.replace("\\\\", "\\"))
    } else {
        Some(inner.to_string())
    }
}

fn split_command_line(command: &str) -> Result<(String, Vec<String>), String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in command.chars() {
        match ch {
            '\'' | '"' => {
                if quote == Some(ch) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(ch);
                } else {
                    current.push(ch);
                }
            }
            c if c.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err("Unclosed quote in MCP server command.".to_string());
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.is_empty() {
        return Err("MCP server command is empty.".to_string());
    }

    Ok((parts.remove(0), parts))
}

fn is_http_endpoint(command: &str) -> bool {
    let trimmed = command.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

fn resolve_server_command(command: &str) -> Result<(String, Vec<String>), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("MCP server command is empty.".to_string());
    }

    let path = Path::new(trimmed);
    if path.exists() && path.is_dir() {
        #[cfg(target_os = "windows")]
        let launcher = "npx.cmd".to_string();
        #[cfg(not(target_os = "windows"))]
        let launcher = "npx".to_string();

        return Ok((
            launcher,
            vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                trimmed.to_string(),
            ],
        ));
    }

    split_command_line(trimmed)
}

async fn handle_http_command(server_url: &str, request: ParsedMcpCommand) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut session_id: Option<String> = None;

    let init_response = send_http_message(
        &client,
        server_url,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {
                    "name": "RustyChat",
                    "version": "0.1.0"
                }
            }
        }),
        session_id.as_deref(),
    )
    .await?;
    session_id = init_response.1;
    if init_response.0.get("error").is_some() {
        return Err(format!(
            "MCP initialize failed: {}",
            serde_json::to_string_pretty(&init_response.0["error"]).unwrap_or_default()
        ));
    }

    let _ = send_http_message(
        &client,
        server_url,
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
        session_id.as_deref(),
    )
    .await;

    match request {
        ParsedMcpCommand::ToolsList => {
            let response = send_http_message(
                &client,
                server_url,
                &json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/list",
                    "params": {}
                }),
                session_id.as_deref(),
            )
            .await?;
            format_tools_response(&response.0)
        }
        ParsedMcpCommand::ToolsCall { name, arguments } => {
            let response = send_http_message(
                &client,
                server_url,
                &json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": name,
                        "arguments": arguments
                    }
                }),
                session_id.as_deref(),
            )
            .await?;
            format_call_response(&name, &response.0)
        }
    }
}

async fn initialize(stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>) -> Result<(), String> {
    write_message(
        stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "RustyChat",
                    "version": "0.1.0"
                }
            }
        }),
    )
    .await?;

    let init_response = read_response(stdout, 1).await?;
    if init_response.get("error").is_some() {
        return Err(format!(
            "MCP initialize failed: {}",
            serde_json::to_string_pretty(&init_response["error"]).unwrap_or_default()
        ));
    }

    write_message(
        stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        }),
    )
    .await?;

    Ok(())
}

async fn tools_list(stdin: &mut ChildStdin, stdout: &mut BufReader<ChildStdout>) -> Result<String, String> {
    write_message(
        stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await?;

    let response = read_response(stdout, 2).await?;
    format_tools_response(&response)
}

async fn tools_call(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    name: &str,
    arguments: Value,
) -> Result<String, String> {
    write_message(
        stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        }),
    )
    .await?;

    let response = read_response(stdout, 3).await?;
    format_call_response(name, &response)
}

fn format_tools_response(response: &Value) -> Result<String, String> {
    if let Some(error) = response.get("error") {
        return Err(format!(
            "tools/list failed: {}",
            serde_json::to_string_pretty(error).unwrap_or_default()
        ));
    }

    let tools = response["result"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if tools.is_empty() {
        return Ok("No MCP tools were returned by the server.".to_string());
    }

    let mut lines = vec!["Available MCP tools:".to_string()];
    for tool in tools {
        let name = tool["name"].as_str().unwrap_or("unknown");
        let description = tool["description"].as_str().unwrap_or("");
        if description.is_empty() {
            lines.push(format!("- {name}"));
        } else {
            lines.push(format!("- {name}: {description}"));
        }
    }
    Ok(lines.join("\n"))
}

fn format_call_response(tool_name: &str, response: &Value) -> Result<String, String> {
    if let Some(error) = response.get("error") {
        return Err(format!(
            "tools/call failed: {}",
            serde_json::to_string_pretty(error).unwrap_or_default()
        ));
    }

    let result = &response["result"];
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        let is_error = result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        let mut sections = Vec::new();

        for item in content {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                let trimmed = text.trim();
                if let Ok(json_value) = serde_json::from_str::<Value>(trimmed) {
                    sections.push(format_tool_json_payload(tool_name, &json_value));
                } else {
                    sections.push(format_tool_text_payload(tool_name, trimmed));
                }
            } else {
                let pretty = serde_json::to_string_pretty(item)
                    .map_err(|e| format!("Failed to format MCP content item: {e}"))?;
                sections.push(format!("```json\n{pretty}\n```"));
            }
        }

        if sections.is_empty() {
            return Ok(if is_error {
                "MCP call returned an error with no text payload.".to_string()
            } else {
                "MCP call returned no content.".to_string()
            });
        }

        if is_error {
            return Ok(format!("MCP tool returned an error:\n\n{}", sections.join("\n\n")));
        }

        return Ok(sections.join("\n\n"));
    }

    serde_json::to_string_pretty(result)
        .map(|pretty| format!("```json\n{pretty}\n```"))
        .map_err(|e| format!("Failed to format MCP tool result: {e}"))
}

fn format_tool_text_payload(tool_name: &str, text: &str) -> String {
    match tool_name {
        "list_directory" | "list_directory_with_sizes" => format_directory_listing(text),
        "read_text_file" | "read_file" => format!("```text\n{}\n```", text),
        _ => text.to_string(),
    }
}

fn format_tool_json_payload(tool_name: &str, value: &Value) -> String {
    match tool_name {
        "directory_tree" => format_directory_tree(value).unwrap_or_else(|| fenced_json(value)),
        "list_allowed_directories" | "search_files" => {
            format_string_list(value).unwrap_or_else(|| fenced_json(value))
        }
        "get_file_info" => format_file_info(value).unwrap_or_else(|| fenced_json(value)),
        _ => fenced_json(value),
    }
}

fn fenced_json(value: &Value) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(pretty) => format!("```json\n{pretty}\n```"),
        Err(_) => "```json\n{}\n```".to_string(),
    }
}

fn format_string_list(value: &Value) -> Option<String> {
    let items = value.as_array()?;
    let entries: Vec<String> = items
        .iter()
        .filter_map(|item| item.as_str().map(|s| format!("- `{s}`")))
        .collect();
    if entries.is_empty() {
        None
    } else {
        Some(entries.join("\n"))
    }
}

fn format_directory_listing(text: &str) -> String {
    let mut lines = Vec::new();
    for raw in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(rest) = raw.strip_prefix("[DIR]") {
            lines.push(format!("- `DIR` {}", rest.trim()));
        } else if let Some(rest) = raw.strip_prefix("[FILE]") {
            lines.push(format!("- `FILE` {}", rest.trim()));
        } else {
            lines.push(format!("- {}", raw));
        }
    }

    if lines.is_empty() {
        text.to_string()
    } else {
        lines.join("\n")
    }
}

fn format_directory_tree(value: &Value) -> Option<String> {
    fn push_node(lines: &mut Vec<String>, node: &Value, depth: usize) {
        let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("file");
        let indent = "  ".repeat(depth);
        let label = if node_type == "directory" { "DIR" } else { "FILE" };
        lines.push(format!("{indent}- `{label}` {name}"));

        if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
            for child in children {
                push_node(lines, child, depth + 1);
            }
        }
    }

    let nodes = value.as_array()?;
    let mut lines = Vec::new();
    for node in nodes {
        push_node(&mut lines, node, 0);
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn format_file_info(value: &Value) -> Option<String> {
    let map = value.as_object()?;
    let mut lines = Vec::new();
    for key in ["path", "type", "size", "created", "modified", "permissions"] {
        if let Some(entry) = map.get(key) {
            let rendered = entry
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| entry.to_string());
            lines.push(format!("- **{}**: {}", key, rendered));
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

async fn send_http_message(
    client: &reqwest::Client,
    server_url: &str,
    payload: &Value,
    session_id: Option<&str>,
) -> Result<(Value, Option<String>), String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    if let Some(session_id) = session_id {
        headers.insert(
            HeaderName::from_static("mcp-session-id"),
            HeaderValue::from_str(session_id).map_err(|e| format!("Invalid MCP session id: {e}"))?,
        );
    }

    let response = client
        .post(server_url)
        .headers(headers)
        .json(payload)
        .send()
        .await
        .map_err(|e| format!("Failed to reach MCP HTTP endpoint: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("MCP HTTP endpoint returned status {}", response.status()));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let session_id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let body = if content_type.contains("text/event-stream") {
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read MCP SSE response: {e}"))?;
        parse_sse_json_response(&text)?
    } else {
        response
            .json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse MCP HTTP response: {e}"))?
    };
    Ok((body, session_id))
}

async fn write_message(stdin: &mut ChildStdin, payload: &Value) -> Result<(), String> {
    let body = serde_json::to_string(payload).map_err(|e| format!("Failed to serialize MCP payload: {e}"))?;
    stdin
        .write_all(body.as_bytes())
        .await
        .map_err(|e| format!("Failed to write MCP body: {e}"))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to write MCP newline delimiter: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Failed to flush MCP stdin: {e}"))
}

async fn read_response(stdout: &mut BufReader<ChildStdout>, expected_id: i64) -> Result<Value, String> {
    let expected_id_json = json!(expected_id);
    loop {
        let mut line = String::new();
        match stdout.read_line(&mut line).await {
            Ok(0) => {
                return Err("MCP server closed stdout before sending a response.".to_string());
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let message: Value = serde_json::from_str(trimmed)
                    .map_err(|e| format!("Failed to parse MCP JSON line: {e}. Line: {trimmed}"))?;
                if message.get("id") == Some(&expected_id_json) {
                    return Ok(message);
                }
            }
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                return Err("MCP server closed stdout before sending a full response.".to_string());
            }
            Err(e) => return Err(format!("Failed to read MCP stdout: {e}")),
        }
    }
}

fn parse_sse_json_response(body: &str) -> Result<Value, String> {
    let mut data_lines = Vec::new();
    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            let payload = data.trim();
            if !payload.is_empty() {
                data_lines.push(payload.to_string());
            }
        }
    }

    if data_lines.is_empty() {
        return Err("MCP SSE response did not include any JSON data events.".to_string());
    }

    for payload in data_lines {
        if let Ok(json) = serde_json::from_str::<Value>(&payload) {
            return Ok(json);
        }
    }

    Err("Failed to parse JSON from MCP SSE response.".to_string())
}
