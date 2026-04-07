use dioxus::prelude::*;
use reqwest::Client;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use base64::Engine;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;
use rfd::FileDialog;

use crate::db::{
    clear_document_chunks, count_chat_messages, count_document_chunks, count_indexed_files,
    enforce_history_limit, init_db, load_chat_messages, log_app_error, save_settings, Settings,
    clamp_to_i32, McpKeyValue, McpServerConfig, McpTransport,
};
use crate::mcp::handle_mcp_command;
use crate::ollama::{OllamaChatRequest, OllamaChatResponse, OllamaMessage};
use crate::ui::Markdown;
use crate::rag::{index_directory, get_context};

const MAX_HISTORY_MESSAGES: i64 = 10000;
const MAX_TITLE_LEN: usize = 255;
const ATTACHMENTS_PREFIX: &str = "<rustychat-attachments>";
const ATTACHMENTS_SUFFIX: &str = "</rustychat-attachments>";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastNotification {
    id: String,
    kind: ToastKind,
    title: String,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ChatAttachment {
    path: String,
    name: String,
    kind: AttachmentKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum AttachmentKind {
    File,
    Folder,
    Image,
}

fn make_attachment(path: &Path) -> ChatAttachment {
    let path_str = path.to_string_lossy().to_string();
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&path_str)
        .to_string();
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = if matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp"
    ) {
        AttachmentKind::Image
    } else if path.is_dir() {
        AttachmentKind::Folder
    } else {
        AttachmentKind::File
    };

    ChatAttachment {
        path: path_str,
        name,
        kind,
    }
}

fn serialize_message_payload(text: &str, attachments: &[ChatAttachment]) -> String {
    if attachments.is_empty() {
        return text.to_string();
    }

    let json = serde_json::to_string(attachments).unwrap_or_else(|_| "[]".to_string());
    format!("{ATTACHMENTS_PREFIX}{json}{ATTACHMENTS_SUFFIX}\n{text}")
}

fn parse_message_payload(content: &str) -> (Vec<ChatAttachment>, String) {
    if let Some(start) = content.find(ATTACHMENTS_PREFIX) {
        let after_start = start + ATTACHMENTS_PREFIX.len();
        if let Some(relative_end) = content[after_start..].find(ATTACHMENTS_SUFFIX) {
            let end = after_start + relative_end;
            let attachment_json = &content[after_start..end];
            let attachments = serde_json::from_str::<Vec<ChatAttachment>>(attachment_json)
                .unwrap_or_default();
            let body = content[end + ATTACHMENTS_SUFFIX.len()..]
                .strip_prefix('\n')
                .unwrap_or(&content[end + ATTACHMENTS_SUFFIX.len()..])
                .to_string();
            return (attachments, body);
        }
    }

    (Vec::new(), content.to_string())
}

fn is_prompt_text_file(path: &str) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    matches!(
        extension.as_str(),
        "txt"
            | "md"
            | "rs"
            | "py"
            | "js"
            | "ts"
            | "json"
            | "toml"
            | "c"
            | "cpp"
            | "h"
            | "html"
            | "css"
            | "csv"
            | "yaml"
            | "yml"
            | "xml"
    )
}

fn attachment_image_src(path: &str) -> String {
    let mime = match Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        _ => return format!("file:///{}", path.replace('\\', "/")),
    };

    match std::fs::read(path) {
        Ok(bytes) => {
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            format!("data:{mime};base64,{encoded}")
        }
        Err(_) => format!("file:///{}", path.replace('\\', "/")),
    }
}

fn build_attachment_prompt(attachments: &[ChatAttachment]) -> String {
    if attachments.is_empty() {
        return String::new();
    }

    const MAX_FILE_CHARS: usize = 12_000;
    let mut sections = Vec::new();

    for attachment in attachments {
        match attachment.kind {
            AttachmentKind::Image => {
                sections.push(format!(
                    "Attached image: {} ({})",
                    attachment.name, attachment.path
                ));
            }
            AttachmentKind::Folder => {
                sections.push(build_folder_attachment_prompt(attachment));
            }
            AttachmentKind::File => {
                if is_prompt_text_file(&attachment.path) {
                    match std::fs::read_to_string(&attachment.path) {
                        Ok(mut content) => {
                            if content.len() > MAX_FILE_CHARS {
                                content.truncate(MAX_FILE_CHARS);
                                content.push_str("\n[truncated]");
                            }
                            sections.push(format!(
                                "Attached file: {} ({})\n```text\n{}\n```",
                                attachment.name, attachment.path, content
                            ));
                        }
                        Err(_) => {
                            sections.push(format!(
                                "Attached file: {} ({}) [could not read as text]",
                                attachment.name, attachment.path
                            ));
                        }
                    }
                } else {
                    sections.push(format!(
                        "Attached file: {} ({}) [binary or unsupported preview type]",
                        attachment.name, attachment.path
                    ));
                }
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else {
        format!("Attached items:\n{}", sections.join("\n\n"))
    }
}

fn build_folder_attachment_prompt(attachment: &ChatAttachment) -> String {
    const MAX_FOLDER_FILES: usize = 24;
    const MAX_FOLDER_PREVIEWS: usize = 4;
    const MAX_FOLDER_CHARS: usize = 2_500;

    let mut listed_paths = Vec::new();
    let mut previews = Vec::new();

    for entry in walkdir::WalkDir::new(&attachment.path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
    {
        if listed_paths.len() >= MAX_FOLDER_FILES {
            break;
        }
        let entry_path = entry.path().to_string_lossy().to_string();
        listed_paths.push(entry_path.clone());

        if previews.len() < MAX_FOLDER_PREVIEWS && is_prompt_text_file(&entry_path) {
            if let Ok(mut content) = std::fs::read_to_string(&entry_path) {
                if content.len() > MAX_FOLDER_CHARS {
                    content.truncate(MAX_FOLDER_CHARS);
                    content.push_str("\n[truncated]");
                }
                let name = entry
                    .path()
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or(&entry_path);
                previews.push(format!("Preview from {}:\n```text\n{}\n```", name, content));
            }
        }
    }

    let mut sections = vec![format!(
        "Attached folder: {} ({})",
        attachment.name, attachment.path
    )];

    if listed_paths.is_empty() {
        sections.push("The folder does not contain readable files.".to_string());
    } else {
        sections.push(format!(
            "Folder contents:\n{}",
            listed_paths
                .iter()
                .map(|path| format!("- {}", path))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if !previews.is_empty() {
        sections.push(previews.join("\n\n"));
    }

    sections.join("\n\n")
}

fn attachment_image_payload(path: &str) -> Option<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp") {
        return None;
    }

    std::fs::read(path)
        .ok()
        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn build_attachment_images(attachments: &[ChatAttachment]) -> Vec<String> {
    attachments
        .iter()
        .filter(|attachment| attachment.kind == AttachmentKind::Image)
        .filter_map(|attachment| attachment_image_payload(&attachment.path))
        .collect()
}

fn render_message_for_model(content: &str) -> (String, Vec<String>) {
    let (attachments, body) = parse_message_payload(content);
    let attachment_context = build_attachment_prompt(&attachments);
    let images = build_attachment_images(&attachments);

    let text = match (body.trim().is_empty(), attachment_context.is_empty()) {
        (false, true) => body,
        (true, false) => attachment_context,
        (false, false) => format!("{attachment_context}\n\nMessage:\n{body}"),
        (true, true) => String::new(),
    };

    (text, images)
}

fn push_toast(
    mut toasts: Signal<Vec<ToastNotification>>,
    kind: ToastKind,
    title: impl Into<String>,
    message: impl Into<String>,
) {
    let id = Uuid::new_v4().to_string();
    toasts.push(ToastNotification {
        id: id.clone(),
        kind,
        title: title.into(),
        message: message.into(),
    });

    spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        toasts.retain(|toast| toast.id != id);
    });
}

fn persist_chat_message(conn: &Connection, chat_id: &str, role: &str, content: &str) {
    let _ = conn.execute(
        "INSERT INTO messages (chat_id, role, content) VALUES (?1, ?2, ?3)",
        params![chat_id, role, content],
    );
    enforce_history_limit(conn, chat_id, MAX_HISTORY_MESSAGES);
}

fn refresh_visible_chat_window(
    current_chat_id: Signal<Option<String>>,
    mut messages: Signal<Vec<(String, String)>>,
    mut message_count: Signal<usize>,
    chat_id: &str,
    window_limit: usize,
) {
    if current_chat_id()
        .as_ref()
        .map(|current| current == chat_id)
        .unwrap_or(false)
    {
        let conn = init_db();
        messages.set(load_chat_messages(&conn, chat_id, window_limit as i64));
        message_count.set(count_chat_messages(&conn, chat_id) as usize);
    }
}

fn persist_and_push_message(
    current_chat_id: Signal<Option<String>>,
    messages: Signal<Vec<(String, String)>>,
    message_count: Signal<usize>,
    chat_id: &str,
    role: &str,
    content: impl Into<String>,
    window_limit: usize,
) {
    let content = content.into();
    let conn = init_db();
    persist_chat_message(&conn, chat_id, role, &content);
    refresh_visible_chat_window(
        current_chat_id,
        messages,
        message_count,
        chat_id,
        window_limit,
    );
}

fn record_ui_error(source: &str, message: impl AsRef<str>) {
    let conn = init_db();
    log_app_error(&conn, source, message.as_ref());
}

fn parse_mcp_tools_listing(listing: &str) -> Vec<(String, String)> {
    listing
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let body = trimmed.strip_prefix("- ")?;
            if let Some((name, description)) = body.split_once(':') {
                Some((name.trim().to_string(), description.trim().to_string()))
            } else {
                Some((body.trim().to_string(), String::new()))
            }
        })
        .collect()
}

fn is_path_only_mcp_tool(tool: &str) -> bool {
    matches!(
        tool,
        "read_file"
            | "read_text_file"
            | "read_media_file"
            | "write_file"
            | "create_directory"
            | "list_directory"
            | "list_directory_with_sizes"
            | "directory_tree"
            | "get_file_info"
    )
}

fn build_friendly_mcp_command(tool: &str, raw_args: &str) -> Result<String, String> {
    let trimmed = raw_args.trim();
    if trimmed.is_empty() {
        return Ok(format!("/mcp call {tool} {{}}"));
    }

    if trimmed.starts_with('{') {
        return Ok(format!("/mcp call {tool} {trimmed}"));
    }

    if is_path_only_mcp_tool(tool) {
        let args = serde_json::json!({ "path": trimmed });
        return Ok(format!("/mcp call {tool} {}", args));
    }

    match tool {
        "search_files" => {
            let (path, pattern) = trimmed
                .split_once('|')
                .ok_or_else(|| "For search_files, use `folder path | pattern`, or enter full JSON.".to_string())?;
            let args = serde_json::json!({
                "path": path.trim(),
                "pattern": pattern.trim()
            });
            Ok(format!("/mcp call {tool} {}", args))
        }
        "read_multiple_files" => {
            let paths: Vec<String> = trimmed
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect();
            if paths.is_empty() {
                Err("Add one file path per line, or enter full JSON.".to_string())
            } else {
                let args = serde_json::json!({ "paths": paths });
                Ok(format!("/mcp call {tool} {}", args))
            }
        }
        "move_file" => {
            let (source, destination) = trimmed
                .split_once("->")
                .ok_or_else(|| "For move_file, use `source path -> destination path`, or enter full JSON.".to_string())?;
            let args = serde_json::json!({
                "source": source.trim(),
                "destination": destination.trim()
            });
            Ok(format!("/mcp call {tool} {}", args))
        }
        _ => Err("This tool needs JSON arguments. Use the JSON format shown in its MCP docs or load a simpler filesystem tool.".to_string()),
    }
}

fn mcp_tool_example(tool: &str) -> &'static str {
    match tool {
        "list_allowed_directories" => "No input needed. Leave the box empty and run the tool.",
        "list_directory" | "list_directory_with_sizes" | "directory_tree" => {
            r#"Example:
C:\Users\tella\Documents\AI-Terminal\mcp"#
        }
        "read_text_file" | "read_file" | "read_media_file" | "get_file_info" => {
            r#"Example:
C:\Users\tella\Documents\AI-Terminal\mcp\README.md"#
        }
        "search_files" => {
            r#"Example:
C:\Users\tella\Documents\AI-Terminal\mcp | **/*.md"#
        }
        "read_multiple_files" => {
            r#"Example:
C:\Users\tella\Documents\AI-Terminal\mcp\README.md
C:\Users\tella\Documents\AI-Terminal\mcp\package.json"#
        }
        "move_file" => {
            r#"Example:
C:\Users\tella\Documents\AI-Terminal\mcp\old.txt -> C:\Users\tella\Documents\AI-Terminal\mcp\new.txt"#
        }
        "create_directory" => {
            r#"Example:
C:\Users\tella\Documents\AI-Terminal\mcp\notes"#
        }
        _ => "Use simple input when available. If this tool needs structured data, paste full JSON instead.",
    }
}

fn parse_mcp_tools_message(content: &str) -> Option<Vec<(String, String)>> {
    let trimmed = content.trim();
    let body = trimmed.strip_prefix("Available MCP tools:")?.trim();
    let mut tools = Vec::new();
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let entry = line.strip_prefix("- ")?;
        if let Some((name, description)) = entry.split_once(':') {
            tools.push((name.trim().to_string(), description.trim().to_string()));
        } else {
            tools.push((entry.trim().to_string(), String::new()));
        }
    }
    if tools.is_empty() { None } else { Some(tools) }
}

fn parse_mcp_file_rows(content: &str) -> Option<Vec<(String, String)>> {
    let mut rows = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let entry = line.strip_prefix("- ")?;
        if let Some(name) = entry.strip_prefix("`DIR` ") {
            rows.push(("DIR".to_string(), name.trim().to_string()));
        } else if let Some(name) = entry.strip_prefix("`FILE` ") {
            rows.push(("FILE".to_string(), name.trim().to_string()));
        } else {
            return None;
        }
    }
    if rows.is_empty() { None } else { Some(rows) }
}

fn parse_mcp_info_rows(content: &str) -> Option<Vec<(String, String)>> {
    let mut rows = Vec::new();
    for line in content.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let entry = line.strip_prefix("- **")?;
        let (key, value) = entry.split_once("**: ")?;
        rows.push((key.trim().to_string(), value.trim().to_string()));
    }
    if rows.is_empty() { None } else { Some(rows) }
}

fn serialize_mcp_pairs(entries: &[McpKeyValue]) -> String {
    entries
        .iter()
        .filter(|entry| !entry.key.trim().is_empty())
        .map(|entry| format!("{}={}", entry.key, entry.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_mcp_pairs(input: &str, label: &str) -> Result<Vec<McpKeyValue>, String> {
    let mut entries = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, value) = match trimmed.split_once('=') {
            Some((key, value)) => (key.trim(), value.trim()),
            None => (trimmed, ""),
        };
        if key.is_empty() {
            return Err(format!("{label} line {} is missing a key.", idx + 1));
        }
        if !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            return Err(format!(
                "{label} key `{key}` contains unsupported characters."
            ));
        }
        entries.push(McpKeyValue {
            key: key.to_string(),
            value: value.to_string(),
        });
    }
    Ok(entries)
}

fn validate_mcp_server(server: &McpServerConfig) -> Vec<String> {
    let mut errors = Vec::new();
    if server.name.trim().is_empty() {
        errors.push("Server name is required.".to_string());
    }
    if server.target.trim().is_empty() {
        errors.push("Connection target is required.".to_string());
    }

    match server.transport {
        McpTransport::Http => {
            if !server.target.starts_with("http://") && !server.target.starts_with("https://") {
                errors.push("HTTP transport requires an http:// or https:// URL.".to_string());
            }
        }
        McpTransport::Filesystem => {
            if server.target.contains("http://") || server.target.contains("https://") {
                errors.push("Filesystem transport expects a folder path, not a URL.".to_string());
            }
        }
        McpTransport::Stdio => {}
    }

    if server.auth_header_name.trim().is_empty() ^ server.auth_token.trim().is_empty() {
        errors.push("Auth header name and auth token must either both be set or both be empty.".to_string());
    }

    errors
}

fn transport_label(transport: &McpTransport) -> &'static str {
    match transport {
        McpTransport::Stdio => "StdIO",
        McpTransport::Http => "HTTP",
        McpTransport::Filesystem => "Filesystem",
    }
}

fn mcp_target_placeholder(transport: &McpTransport) -> &'static str {
    match transport {
        McpTransport::Stdio => "npx -y @modelcontextprotocol/server-filesystem C:\\path",
        McpTransport::Http => "https://your-mcp-server.example.com/mcp",
        McpTransport::Filesystem => "C:\\Users\\tella\\Documents\\project",
    }
}

/* ================= SETTINGS MODAL ================= */

#[component]
pub fn SettingsModal(
    settings: Signal<Settings>,
    show_settings: Signal<bool>,
    chats: Signal<Vec<(String, String)>>,
    messages: Signal<Vec<(String, String)>>,
    message_count: Signal<usize>,
    current_chat_id: Signal<Option<String>>,
    toasts: Signal<Vec<ToastNotification>>,
) -> Element {
    // local editable copies using signals
    let mut local_model = use_signal(|| settings().model.clone());
    let mut local_embed_model = use_signal(|| settings().embed_model.clone());
    let mut local_mcp_servers = use_signal(|| settings().mcp_servers.clone());
    let mut local_active_mcp_server_id = use_signal(|| settings().active_mcp_server_id.clone());
    let mut local_selected_mcp_server_id = use_signal(|| settings().active_mcp_server_id.clone());
    let mut local_system = use_signal(|| settings().system_prompt.clone());
    let mut local_allow_code_execution = use_signal(|| settings().allow_code_execution);
    let mut local_execution_timeout_secs = use_signal(|| settings().execution_timeout_secs);
    let mut local_temp = use_signal(|| settings().temperature);
    let mut local_top_p = use_signal(|| settings().top_p);
    let mut local_max_tokens = use_signal(|| settings().max_tokens);
    let mut local_zoom = use_signal(|| settings().zoom);
    let local_width = use_signal(|| settings().window_width);
    let local_height = use_signal(|| settings().window_height);

    // list of available models from Ollama
    let available_models = use_signal(|| Vec::<String>::new());

    // fetch available models when modal mounts
    {
        let mut models_sig = available_models.clone();
        use_effect(move || {
            spawn(async move {
                let client = Client::new();
                let url = "http://localhost:11434/api/tags";
                if let Ok(resp) = client.get(url).send().await {
                    if let Ok(json) = resp.json::<Value>().await {
                        let mut names: Vec<String> = Vec::new();

                        if let Some(models_arr) = json.get("models").and_then(|v| v.as_array()) {
                            for item in models_arr {
                                if let Some(m) = item
                                    .get("model")
                                    .or(item.get("name"))
                                    .and_then(|v| v.as_str())
                                {
                                    names.push(m.to_string());
                                }
                            }
                        } else if let Some(arr) = json.as_array() {
                            for item in arr {
                                if let Some(s) = item.as_str() {
                                    names.push(s.to_string());
                                } else if let Some(n) = item.get("name").and_then(|v| v.as_str()) {
                                    names.push(n.to_string());
                                } else if let Some(n) = item.get("model").and_then(|v| v.as_str()) {
                                    names.push(n.to_string());
                                }
                            }
                        }

                        let mut seen = std::collections::HashSet::new();
                        names.retain(|n| seen.insert(n.clone()));

                        models_sig.set(names);
                    }
                }
            });
        });
    }

    {
        let show_settings_sig = show_settings.clone();
        let settings_sig = settings.clone();
        let mut local_model_sig = local_model.clone();
        let mut local_embed_model_sig = local_embed_model.clone();
        let mut local_mcp_servers_sig = local_mcp_servers.clone();
        let mut local_active_mcp_server_id_sig = local_active_mcp_server_id.clone();
        let mut local_selected_mcp_server_id_sig = local_selected_mcp_server_id.clone();
        let mut local_system_sig = local_system.clone();
        let mut local_allow_code_execution_sig = local_allow_code_execution.clone();
        let mut local_execution_timeout_secs_sig = local_execution_timeout_secs.clone();
        let mut local_temp_sig = local_temp.clone();
        let mut local_top_p_sig = local_top_p.clone();
        let mut local_max_tokens_sig = local_max_tokens.clone();
        let mut local_zoom_sig = local_zoom.clone();
        let mut local_width_sig = local_width.clone();
        let mut local_height_sig = local_height.clone();
        use_effect(move || {
            if show_settings_sig() {
                let s = settings_sig();
                local_model_sig.set(s.model.clone());
                local_embed_model_sig.set(s.embed_model.clone());
                local_mcp_servers_sig.set(s.mcp_servers.clone());
                local_active_mcp_server_id_sig.set(s.active_mcp_server_id.clone());
                local_selected_mcp_server_id_sig.set(s.active_mcp_server_id.clone());
                local_system_sig.set(s.system_prompt.clone());
                local_allow_code_execution_sig.set(s.allow_code_execution);
                local_execution_timeout_secs_sig.set(s.execution_timeout_secs);
                local_temp_sig.set(s.temperature);
                local_top_p_sig.set(s.top_p);
                local_max_tokens_sig.set(s.max_tokens);
                local_zoom_sig.set(s.zoom);
                local_width_sig.set(s.window_width);
                local_height_sig.set(s.window_height);
            }
        });
    }

    let options_vec = {
        let mut v = available_models().clone();
        for selected in [local_model().clone(), local_embed_model().clone()] {
            if !selected.is_empty() && !v.iter().any(|s| s == &selected) {
                v.insert(0, selected);
            }
        }
        v
    };

    let apply = {
        to_owned![
            local_model,
            local_embed_model,
            local_mcp_servers,
            local_active_mcp_server_id,
            local_system,
            local_allow_code_execution,
            local_execution_timeout_secs,
            local_temp,
            local_top_p,
            local_max_tokens,
            local_zoom,
            local_width,
            local_height,
            settings,
            show_settings
        ];
        move |_| {
            let mut model_str = local_model().clone();
            model_str = model_str.trim().to_string();
            let mut embed_model_str = local_embed_model().clone();
            embed_model_str = embed_model_str.trim().to_string();
            let mut mcp_servers = local_mcp_servers();
            for server in &mut mcp_servers {
                server.name = server.name.trim().to_string();
                server.target = server.target.trim().to_string();
                server.auth_header_name = server.auth_header_name.trim().to_string();
                server.auth_token = server.auth_token.trim().to_string();
                server.custom_headers.retain(|entry| !entry.key.trim().is_empty());
                server.env_vars.retain(|entry| !entry.key.trim().is_empty());
            }
            let active_mcp_server_id = if mcp_servers
                .iter()
                .any(|server| server.id == local_active_mcp_server_id())
            {
                local_active_mcp_server_id()
            } else {
                mcp_servers
                    .first()
                    .map(|server| server.id.clone())
                    .unwrap_or_default()
            };

            let new_settings = Settings {
                model: model_str,
                embed_model: embed_model_str,
                mcp_server_command: String::new(),
                mcp_servers,
                active_mcp_server_id,
                system_prompt: local_system().clone(),
                allow_code_execution: local_allow_code_execution(),
                execution_timeout_secs: local_execution_timeout_secs().clamp(3, 120),
                temperature: local_temp(),
                top_p: local_top_p(),
                max_tokens: clamp_to_i32(local_max_tokens().into()),
                zoom: clamp_to_i32(local_zoom().into()),
                maximized: true,
                window_width: clamp_to_i32(local_width().into()),
                window_height: clamp_to_i32(local_height().into()),
            };
            let conn = init_db();
            save_settings(&conn, &new_settings);
            settings.set(new_settings);
            show_settings.set(false);
            push_toast(
                toasts,
                ToastKind::Success,
                "Settings updated",
                "Your model, MCP, and app preferences were saved.",
            );
        }
    };

    let delete_all = {
        to_owned![chats, messages, message_count, current_chat_id, show_settings];
        move |_| {
            let conn = init_db();
            conn.execute("DELETE FROM messages", []).ok();
            conn.execute("DELETE FROM chats", []).ok();

            chats.set(vec![]);
            messages.set(vec![]);
            message_count.set(0);
            current_chat_id.set(None);
            show_settings.set(false);
        }
    };

    let cancel = {
        to_owned![show_settings];
        move |_| {
            show_settings.set(false);
        }
    };

    let selected_server_id = local_selected_mcp_server_id();
    let selected_server = local_mcp_servers()
        .iter()
        .find(|server| server.id == selected_server_id)
        .cloned()
        .or_else(|| local_mcp_servers().first().cloned());
    let selected_server_errors = selected_server
        .as_ref()
        .map(validate_mcp_server)
        .unwrap_or_default();
    let mut name_counts = std::collections::HashMap::<String, usize>::new();
    for server in local_mcp_servers() {
        let key = server.name.trim().to_lowercase();
        if !key.is_empty() {
            *name_counts.entry(key).or_insert(0) += 1;
        }
    }
    let has_duplicate_names = name_counts.values().any(|count| *count > 1);
    let has_mcp_validation_errors = has_duplicate_names
        || local_mcp_servers()
            .iter()
            .any(|server| !validate_mcp_server(server).is_empty());

    rsx! {
        div { class: "settings-overlay",
            div { class: "settings-modal",
                h3 { "Settings" }

                label { "Model (choose one of the available Ollama models)" }
                select {
                    class: "input",
                    value: "{local_model}",
                    onchange: move |e| local_model.set(e.value()),
                    option { selected: local_model().is_empty(), value: "", "- Select a model -" }
                    {options_vec.iter().map(|m| rsx!( option { selected: m == &local_model(), value: "{m}", "{m}" } ))}
                }

                if local_model().is_empty() {
                    p { class: "dim-text warning-text", "No model selected - pick a model to allow sending messages." }
                }

                label { "Embedding model (used for RAG indexing and retrieval)" }
                select {
                    class: "input",
                    value: "{local_embed_model}",
                    onchange: move |e| local_embed_model.set(e.value()),
                    option { selected: local_embed_model().is_empty(), value: "", "- Select an embedding model -" }
                    {options_vec.iter().map(|m| rsx!( option { selected: m == &local_embed_model(), value: "{m}", "{m}" } ))}
                }

                if local_embed_model().is_empty() {
                    p { class: "dim-text warning-text", "No embedding model selected - chat will still work, but RAG indexing and retrieval stay disabled." }
                }

                label { "MCP integrations" }
                div { class: "mcp-settings-shell",
                    div { class: "mcp-server-list",
                        div { class: "mcp-settings-toolbar",
                            p { class: "dim-text warning-text", "Save multiple MCP integrations, choose an active one, and add auth headers or env vars without keeping everything in one raw command string." }
                            button {
                                class: "secondary-action-btn",
                                onclick: move |_| {
                                    let id = Uuid::new_v4().to_string();
                                    let next = McpServerConfig {
                                        id: id.clone(),
                                        name: format!("MCP Server {}", local_mcp_servers().len() + 1),
                                        transport: McpTransport::Filesystem,
                                        target: String::new(),
                                        auth_header_name: String::new(),
                                        auth_token: String::new(),
                                        custom_headers: Vec::new(),
                                        env_vars: Vec::new(),
                                    };
                                    let mut servers = local_mcp_servers();
                                    servers.push(next);
                                    local_mcp_servers.set(servers);
                                    local_active_mcp_server_id.set(id.clone());
                                    local_selected_mcp_server_id.set(id);
                                },
                                "Add MCP Server"
                            }
                        }

                        if local_mcp_servers().is_empty() {
                            div { class: "mcp-server-empty",
                                strong { "No MCP integrations yet" }
                                p { "Add one for filesystem browsing, stdio commands, or HTTP endpoints." }
                            }
                        } else {
                            {local_mcp_servers().iter().map(|server| {
                                let server_id = server.id.clone();
                                let is_selected = selected_server_id == server.id;
                                let is_active = local_active_mcp_server_id() == server.id;
                                let duplicate_name = !server.name.trim().is_empty()
                                    && name_counts
                                        .get(&server.name.trim().to_lowercase())
                                        .copied()
                                        .unwrap_or(0) > 1;
                                rsx!(
                                    div {
                                        class: if is_selected { "mcp-server-card selected" } else { "mcp-server-card" },
                                        onclick: {
                                            let server_id = server_id.clone();
                                            move |_| local_selected_mcp_server_id.set(server_id.clone())
                                        },
                                        div { class: "mcp-server-card-top",
                                            strong { "{server.name}" }
                                            span { class: "mcp-server-transport", "{transport_label(&server.transport)}" }
                                        }
                                        p { class: "mcp-server-target", "{server.target}" }
                                        div { class: "mcp-server-card-bottom",
                                            if is_active {
                                                span { class: "mcp-server-badge active", "Active" }
                                            }
                                            if duplicate_name {
                                                span { class: "mcp-server-badge warning", "Duplicate name" }
                                            }
                                            button {
                                                class: "mcp-server-remove",
                                                onclick: {
                                                    let server_id = server_id.clone();
                                                    move |evt| {
                                                    evt.stop_propagation();
                                                    let remaining: Vec<McpServerConfig> = local_mcp_servers()
                                                        .into_iter()
                                                        .filter(|item| item.id != server_id)
                                                        .collect();
                                                    let next_active = if local_active_mcp_server_id() == server_id {
                                                        remaining.first().map(|item| item.id.clone()).unwrap_or_default()
                                                    } else {
                                                        local_active_mcp_server_id()
                                                    };
                                                    let next_selected = remaining.first().map(|item| item.id.clone()).unwrap_or_default();
                                                    local_mcp_servers.set(remaining);
                                                    local_active_mcp_server_id.set(next_active);
                                                    local_selected_mcp_server_id.set(next_selected);
                                                    }
                                                },
                                                "Remove"
                                            }
                                        }
                                    }
                                )
                            })}
                        }
                    }

                    if let Some(server) = selected_server {
                        div { class: "mcp-server-editor",
                            label { "Server name" }
                            input {
                                class: "input",
                                value: "{server.name}",
                                oninput: {
                                    let server_id = server.id.clone();
                                    move |e| {
                                    let mut servers = local_mcp_servers();
                                    if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                        item.name = e.value();
                                    }
                                    local_mcp_servers.set(servers);
                                    }
                                },
                            }

                            label { "Transport" }
                            select {
                                class: "input",
                                value: if matches!(server.transport, McpTransport::Stdio) {
                                    "stdio"
                                } else if matches!(server.transport, McpTransport::Http) {
                                    "http"
                                } else {
                                    "filesystem"
                                },
                                onchange: {
                                    let server_id = server.id.clone();
                                    move |e| {
                                    let mut servers = local_mcp_servers();
                                    if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                        item.transport = match e.value().as_str() {
                                            "http" => McpTransport::Http,
                                            "filesystem" => McpTransport::Filesystem,
                                            _ => McpTransport::Stdio,
                                        };
                                    }
                                    local_mcp_servers.set(servers);
                                    }
                                },
                                option { value: "filesystem", "Filesystem" }
                                option { value: "stdio", "StdIO command" }
                                option { value: "http", "HTTP endpoint" }
                            }

                            label { "Connection target" }
                            input {
                                class: "input",
                                value: "{server.target}",
                                placeholder: "{mcp_target_placeholder(&server.transport)}",
                                oninput: {
                                    let server_id = server.id.clone();
                                    move |e| {
                                    let mut servers = local_mcp_servers();
                                    if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                        item.target = e.value();
                                    }
                                    local_mcp_servers.set(servers);
                                    }
                                },
                            }

                            div { class: "mcp-editor-grid",
                                div {
                                    label { "Auth header name" }
                                    input {
                                        class: "input",
                                        value: "{server.auth_header_name}",
                                        placeholder: "Authorization",
                                        oninput: {
                                            let server_id = server.id.clone();
                                            move |e| {
                                            let mut servers = local_mcp_servers();
                                            if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                                item.auth_header_name = e.value();
                                            }
                                            local_mcp_servers.set(servers);
                                            }
                                        },
                                    }
                                }
                                div {
                                    label { "Auth token / value" }
                                    input {
                                        class: "input",
                                        value: "{server.auth_token}",
                                        placeholder: "Bearer <token>",
                                        oninput: {
                                            let server_id = server.id.clone();
                                            move |e| {
                                            let mut servers = local_mcp_servers();
                                            if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                                item.auth_token = e.value();
                                            }
                                            local_mcp_servers.set(servers);
                                            }
                                        },
                                    }
                                }
                            }

                            label { "Custom headers" }
                            textarea {
                                class: "textarea",
                                value: "{serialize_mcp_pairs(&server.custom_headers)}",
                                placeholder: "X-Org-Id=acme\nX-Workspace=dev",
                                oninput: {
                                    let server_id = server.id.clone();
                                    move |e| {
                                    if let Ok(parsed) = parse_mcp_pairs(&e.value(), "Custom header") {
                                        let mut servers = local_mcp_servers();
                                        if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                            item.custom_headers = parsed;
                                        }
                                        local_mcp_servers.set(servers);
                                    }
                                    }
                                },
                            }

                            label { "Environment variables" }
                            textarea {
                                class: "textarea",
                                value: "{serialize_mcp_pairs(&server.env_vars)}",
                                placeholder: "API_KEY=...\nWORKSPACE_ROOT=C:\\project",
                                oninput: {
                                    let server_id = server.id.clone();
                                    move |e| {
                                    if let Ok(parsed) = parse_mcp_pairs(&e.value(), "Environment variable") {
                                        let mut servers = local_mcp_servers();
                                        if let Some(item) = servers.iter_mut().find(|item| item.id == server_id) {
                                            item.env_vars = parsed;
                                        }
                                        local_mcp_servers.set(servers);
                                    }
                                    }
                                },
                            }

                            div { class: "mcp-server-editor-footer",
                                button {
                                    class: if local_active_mcp_server_id() == server.id { "settings-toggle active" } else { "settings-toggle" },
                                    onclick: {
                                        let server_id = server.id.clone();
                                        move |_| local_active_mcp_server_id.set(server_id.clone())
                                    },
                                    if local_active_mcp_server_id() == server.id { "Active integration" } else { "Make active" }
                                }
                                p { class: "dim-text warning-text",
                                    match server.transport {
                                        McpTransport::Filesystem => "Filesystem mode launches the MCP filesystem server against the selected folder.",
                                        McpTransport::Stdio => "StdIO mode launches the command directly and passes env vars into that process.",
                                        McpTransport::Http => "HTTP mode calls the MCP endpoint directly and attaches your auth or custom headers.",
                                    }
                                }
                            }

                            if has_duplicate_names {
                                p { class: "dim-text warning-text", "Each MCP integration should have a distinct name so the active selection is clear." }
                            }
                            {selected_server_errors.iter().map(|error| rsx!(
                                p { class: "dim-text warning-text", "{error}" }
                            ))}
                        }
                    } else {
                        div { class: "mcp-server-editor empty",
                            p { "Select an MCP integration to edit it." }
                        }
                    }
                }

                label { "System prompt (optional)" }
                textarea {
                    class: "textarea",
                    value: "{local_system}",
                    oninput: move |e| local_system.set(e.value()),
                }

                label { "Inline code execution" }
                div { class: "settings-toggle-row",
                    button {
                        class: if local_allow_code_execution() { "settings-toggle active" } else { "settings-toggle" },
                        onclick: move |_| local_allow_code_execution.set(!local_allow_code_execution()),
                        if local_allow_code_execution() { "Enabled" } else { "Disabled" }
                    }
                    p { class: "dim-text warning-text", "Generated code runs locally on your machine. RustyChat now uses a temp working folder and timeout, but this is still not a full sandbox." }
                }

                label { "Execution timeout (seconds)" }
                input {
                    class: "input",
                    r#type: "number",
                    step: "1",
                    min: "3",
                    max: "120",
                    value: "{local_execution_timeout_secs}",
                    oninput: move |e| {
                        let parsed = e.value().parse::<i32>().unwrap_or(12);
                        local_execution_timeout_secs.set(parsed.clamp(3, 120));
                    }
                }

                label { "Temperature" }
                input {
                    class: "input",
                    r#type: "number",
                    step: "0.05",
                    min: "0.0",
                    max: "2.0",
                    value: "{local_temp}",
                    oninput: move |e| local_temp.set(e.value().parse::<f64>().unwrap_or(0.7))
                }

                label { "Top-p" }
                input {
                    class: "input",
                    r#type: "number",
                    step: "0.01",
                    min: "0.0",
                    max: "1.0",
                    value: "{local_top_p}",
                    oninput: move |e| local_top_p.set(e.value().parse::<f64>().unwrap_or(0.95))
                }

                label { "Max tokens (clamped to Rust/i32 limits)" }
                input {
                    class: "input",
                    r#type: "number",
                    step: "1",
                    min: "1",
                    max: format!("{}", i32::MAX),
                    value: "{local_max_tokens}",
                    oninput: move |e| {
                        let parsed = e.value().parse::<i64>().unwrap_or(512);
                        local_max_tokens.set(clamp_to_i32(parsed));
                    }
                }

                label { "Zoom (%) — applied globally (50 - 200)" }
                div { class: "zoom-row",
                    button { onclick: move |_| { local_zoom.set((local_zoom() - 10).max(50)); }, "−" }
                    span { "{local_zoom}%" }
                    button { onclick: move |_| { local_zoom.set((local_zoom() + 10).min(200)); }, "+" }
                }

                div { class: "modal-actions",
                    button { onclick: delete_all, class: "delete-all", "Delete All History" }
                    button { onclick: apply, disabled: has_mcp_validation_errors, "Apply" }
                    button { onclick: cancel, "Cancel" }
                }
            }
        }
    }
}

/* ================= SIDEBAR ================= */

#[component]
pub fn Sidebar(
    chats: Signal<Vec<(String, String)>>,
    current_chat_id: Signal<Option<String>>,
    messages: Signal<Vec<(String, String)>>,
    message_count: Signal<usize>,
    show_settings: Signal<bool>,
) -> Element {
    let mut editing_chat = use_signal(|| Option::<String>::None);
    let mut edit_text = use_signal(|| "".to_string());
    let mut search_query = use_signal(|| "".to_string());

    let filtered_chats = {
        let query = search_query().to_lowercase();
        chats().into_iter().filter(move |(_, title)| {
            title.to_lowercase().contains(&query)
        }).collect::<Vec<_>>()
    };

    rsx! {
        div { class: "sidebar",
            h1 { class: "logo", "RustyChat" }

            button {
                class: "new-chat-btn big",
                onclick: move |_| {
                    let conn = init_db();
                    let new_id = Uuid::new_v4().to_string();
                    let title = "New Chat".to_string();

                    conn.execute(
                        "INSERT INTO chats (id, title) VALUES (?1, ?2)",
                        params![new_id, title],
                    ).unwrap();

                    chats.push((new_id.clone(), title));
                    current_chat_id.set(Some(new_id));
                    messages.set(vec![]);
                    message_count.set(0);
                    search_query.set("".to_string());
                },
                "➕ New Chat"
            }

            div { class: "search-container",
                input {
                    class: "search-input",
                    placeholder: "Search chats...",
                    value: "{search_query}",
                    oninput: move |e| search_query.set(e.value()),
                }
            }

            div { class: "chat-list",
                {filtered_chats.iter().map(|(id, title)| {
                    let id_owned = id.clone();
                    let title_clone = title.clone();

                    let id_for_open = id_owned.clone();
                    let id_for_save = id_owned.clone();
                    let id_for_rename_btn = id_owned.clone();
                    let id_for_delete = id_owned.clone();

                    let mut chats_handle = chats.clone();
                    let mut messages_handle = messages.clone();
                    let mut message_count_handle = message_count.clone();
                    let mut current_chat_handle = current_chat_id.clone();
                    let mut editing_chat_handle = editing_chat.clone();
                    let mut edit_text_handle = edit_text.clone();

                    rsx! {
                        div { class: "chat-item-row",
                            div {
                                class: {
                                    if current_chat_id() == Some(id_for_open.clone()) {
                                        "chat-item active"
                                    } else {
                                        "chat-item"
                                    }
                                },
                                onclick: move |_| {
                                    let conn = init_db();
                                    messages_handle.set(load_chat_messages(&conn, &id_for_open, 160));
                                    message_count_handle.set(count_chat_messages(&conn, &id_for_open) as usize);
                                    current_chat_handle.set(Some(id_for_open.clone()));
                                },

                                {
                                    if editing_chat_handle().as_ref().map(|c| c == &id_for_save).unwrap_or(false) {
                                        rsx! {
                                            div { class: "rename-row",
                                                input {
                                                    class: "rename-input",
                                                    value: "{edit_text_handle}",
                                                    oninput: move |e| {
                                                        let mut v = e.value();
                                                        if v.len() > MAX_TITLE_LEN {
                                                            v.truncate(MAX_TITLE_LEN);
                                                        }
                                                        edit_text_handle.set(v);
                                                    },
                                                }
                                                button {
                                                    class: "rename-save",
                                                    onclick: move |_| {
                                                        let mut new_title = edit_text_handle().clone();
                                                        if new_title.len() > MAX_TITLE_LEN {
                                                            new_title.truncate(MAX_TITLE_LEN);
                                                        }
                                                        let trimmed = new_title;

                                                        let conn = init_db();
                                                        conn.execute(
                                                            "UPDATE chats SET title = ?1 WHERE id = ?2",
                                                            params![trimmed, id_for_save.clone()],
                                                        ).unwrap();

                                                        chats_handle.set(
                                                            chats_handle().into_iter().map(|(cid, t)| {
                                                                if cid == id_for_save { (cid, trimmed.clone()) } else { (cid, t) }
                                                            }).collect()
                                                        );

                                                        editing_chat_handle.set(None);
                                                    },
                                                    "Save"
                                                }
                                                button {
                                                    class: "rename-cancel",
                                                    onclick: move |_| {
                                                        editing_chat_handle.set(None);
                                                    },
                                                    "Cancel"
                                                }
                                            }
                                        }
                                    } else {
                                        rsx! {
                                            Fragment {
                                                div { class: "chat-title", "{title_clone}" }
                                                div { class: "chat-actions",
                                                    button {
                                                        class: "chat-action-btn rename-btn",
                                                        onclick: move |e| {
                                                            e.stop_propagation();
                                                            editing_chat.set(Some(id_for_rename_btn.clone()));
                                                            let mut init = title_clone.clone();
                                                            if init.len() > MAX_TITLE_LEN {
                                                                init.truncate(MAX_TITLE_LEN);
                                                            }
                                                            edit_text.set(init);
                                                        },
                                                        title: "Rename chat",
                                                        "✎"
                                                    }
                                                    button {
                                                        class: "chat-action-btn delete-chat-btn",
                                                        onclick: move |e| {
                                                            e.stop_propagation();
                                                            let conn = init_db();

                                                            conn.execute(
                                                                "DELETE FROM messages WHERE chat_id = ?1",
                                                                params![id_for_delete.clone()],
                                                            ).unwrap();

                                                            conn.execute(
                                                                "DELETE FROM chats WHERE id = ?1",
                                                                params![id_for_delete.clone()],
                                                            ).unwrap();

                                                            chats_handle.set(
                                                                chats_handle()
                                                                    .into_iter()
                                                                    .filter(|(cid, _)| cid != &id_for_delete)
                                                                    .collect()
                                                            );

                                                            if current_chat_handle() == Some(id_for_delete.clone()) {
                                                                current_chat_handle.set(None);
                                                                messages_handle.set(vec![]);
                                                                message_count_handle.set(0);
                                                            }
                                                        },
                                                        title: "Delete chat",
                                                        "×"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                })}
            }

            div { class: "sidebar-footer",
                button {
                    class: "sidebar-icon-btn settings-btn",
                    onclick: move |_| {
                        show_settings.set(!show_settings());
                    },
                    span { class: "settings-icon", "⚙" }
                    span { class: "settings-tooltip", "Settings" }
                }
                a {
                    class: "sidebar-icon-btn repo-icon",
                    href: "https://github.com/Djeylani/RustyChat",
                    target: "_blank",
                    title: "Open repository",
                    "↗"
                }
            }
        }
    }
}

/* ================= CHAT WINDOW ================= */

#[component]
pub fn ChatWindow(
    current_chat_id: Signal<Option<String>>,
    messages: Signal<Vec<(String, String)>>,
    message_count: Signal<usize>,
    settings: Signal<Settings>,
    chats: Signal<Vec<(String, String)>>,
    toasts: Signal<Vec<ToastNotification>>,
) -> Element {
    let mut input_text = use_signal(|| "".to_string());
    let mut pending_attachments = use_signal(|| Vec::<ChatAttachment>::new());
    let mut show_composer_tools = use_signal(|| false);
    let mut loading_chat = use_signal(|| Option::<String>::None);
    let mut current_cancel = use_signal(|| Option::<Arc<AtomicBool>>::None);
    let mut is_indexing = use_signal(|| false);
    let mut rag_status = use_signal(|| Option::<String>::None);
    let mcp_status = use_signal(|| Option::<String>::None);
    let mcp_tools_cache = use_signal(|| Option::<String>::None);
    let mcp_last_error = use_signal(|| Option::<String>::None);
    let mcp_tool_entries = use_signal(|| Vec::<(String, String)>::new());
    let mut selected_mcp_tool = use_signal(|| "".to_string());
    let mut mcp_tool_args = use_signal(|| "".to_string());
    let mut show_mcp_workspace = use_signal(|| false);
    let mut visible_message_count = use_signal(|| 160_usize);
    const MESSAGE_WINDOW_SIZE: usize = 160;
    let indexed_chunks = use_signal(|| count_document_chunks(&init_db()));
    let indexed_files = use_signal(|| count_indexed_files(&init_db()));
    let http_client = use_signal(|| Client::new());

    // Auto-scroll logic
    use_effect(move || {
        let _msgs = messages();
        let _loading = loading_chat();
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let eval = document::eval(
                r#"
                var container = document.querySelector('.chat-messages');
                if (container) {
                    container.scrollTop = container.scrollHeight;
                }
                "#
            );
            let _ = eval.await;
        });
    });

    {
        let current_chat_id = current_chat_id.clone();
        let mut visible_message_count = visible_message_count.clone();
        use_effect(move || {
            let _current = current_chat_id();
            visible_message_count.set(MESSAGE_WINDOW_SIZE);
        });
    }

    let header_title = {
        if let Some(id) = current_chat_id() {
            chats()
                .iter()
                .find(|(cid, _)| cid == &id)
                .map(|(_, t)| t.clone())
                .unwrap_or(id.clone())
        } else {
            "No Chat Selected".to_string()
        }
    };

    let model_display = {
        let m = settings().model.clone();
        if m.trim().is_empty() {
            "No model selected".to_string()
        } else {
            m
        }
    };

    let embed_model_display = {
        let m = settings().embed_model.clone();
        if m.trim().is_empty() {
            "No embedding model selected".to_string()
        } else {
            m
        }
    };
    let active_mcp_server = settings().active_mcp_server();
    let active_mcp_server_present = active_mcp_server.is_some();
    let active_mcp_server_name = active_mcp_server
        .as_ref()
        .map(|server| server.name.clone())
        .unwrap_or_default();
    let mcp_display = if active_mcp_server.is_none() {
        "MCP: not configured".to_string()
    } else if let Some(status) = mcp_status() {
        format!(
            "MCP: {} · {status}",
            active_mcp_server_name
        )
    } else {
        format!(
            "MCP: {} · ready",
            active_mcp_server_name
        )
    };
    let corpus_display = format!(
        "Indexed corpus: {} files, {} chunks",
        indexed_files(),
        indexed_chunks()
    );
    let mcp_is_busy = matches!(
        mcp_status().as_deref(),
        Some("starting server") | Some("running command")
    );
    let mut refresh_index_metrics = {
        let mut indexed_files = indexed_files.clone();
        let mut indexed_chunks = indexed_chunks.clone();
        move || {
            let conn = init_db();
            indexed_files.set(count_indexed_files(&conn));
            indexed_chunks.set(count_document_chunks(&conn));
        }
    };

    let mut run_mcp_command = {
        to_owned![
            current_chat_id,
            messages,
            message_count,
            visible_message_count,
            rag_status,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error,
            mcp_tool_entries,
            selected_mcp_tool,
            toasts
        ];
        move |chat_id: String, command_text: String, user_echo: Option<String>| {
            if let Some(display_text) = user_echo {
                persist_and_push_message(
                    current_chat_id,
                    messages,
                    message_count,
                    &chat_id,
                    "user",
                    display_text,
                    visible_message_count().max(MESSAGE_WINDOW_SIZE),
                );
            }

            mcp_status.set(Some("starting server".to_string()));
            mcp_last_error.set(None);
            rag_status.set(Some("MCP: starting server...".to_string()));

            spawn(async move {
                mcp_status.set(Some("running command".to_string()));
                rag_status.set(Some("MCP: running command...".to_string()));
                let current_settings = crate::db::load_settings(&init_db());
                let Some(active_server) = current_settings.active_mcp_server() else {
                    record_ui_error("mcp", "No active MCP server is configured.");
                    mcp_status.set(Some("error".to_string()));
                    mcp_last_error.set(Some("No active MCP server is configured.".to_string()));
                    rag_status.set(Some("MCP command failed: no active server configured.".to_string()));
                    push_toast(
                        toasts,
                        ToastKind::Error,
                        "MCP not configured",
                        "Choose an active MCP integration in Settings first.",
                    );
                    return;
                };
                let result = handle_mcp_command(&active_server, &command_text).await;
                let conn = init_db();
                let assistant_text = match result {
                    Ok(output) => {
                        mcp_status.set(Some("ready".to_string()));
                        mcp_last_error.set(None);
                        if command_text.trim() == "/mcp tools"
                            || command_text.trim() == "/mcp tools/list"
                        {
                            let parsed = parse_mcp_tools_listing(&output);
                            if selected_mcp_tool().is_empty() {
                                if let Some((name, _)) = parsed.first() {
                                    selected_mcp_tool.set(name.clone());
                                }
                            }
                            mcp_tool_entries.set(parsed);
                            mcp_tools_cache.set(Some(output.clone()));
                        }
                        rag_status.set(Some("MCP command completed.".to_string()));
                        push_toast(
                            toasts,
                            ToastKind::Success,
                            "MCP complete",
                            "The MCP command finished successfully.",
                        );
                        output
                    }
                    Err(err) => {
                        record_ui_error("mcp", &err);
                        mcp_status.set(Some("error".to_string()));
                        mcp_last_error.set(Some(err.clone()));
                        rag_status.set(Some(format!("MCP command failed: {err}")));
                        push_toast(
                            toasts,
                            ToastKind::Error,
                            "MCP failed",
                            err.clone(),
                        );
                        format!("MCP Error: {err}")
                    }
                };

                persist_chat_message(&conn, &chat_id, "assistant", &assistant_text);
                refresh_visible_chat_window(
                    current_chat_id,
                    messages,
                    message_count,
                    &chat_id,
                    visible_message_count().max(MESSAGE_WINDOW_SIZE),
                );
            });
        }
    };

    let send_to_ollama = {
        to_owned![
            messages,
            message_count,
            visible_message_count,
            http_client,
            loading_chat,
            current_cancel,
            current_chat_id,
            settings,
            rag_status,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error,
            toasts
        ];
        move |chat_id: String,
              history_snapshot: Vec<(String, String)>,
              user_message: String,
              user_images: Vec<String>,
              cancel_flag: Arc<AtomicBool>| {
            async move {
                let s = settings();
                if s.model.trim().is_empty() {
                    let db_msg = "Error: No model selected. Please open Settings and choose a model before sending messages.";
                    record_ui_error("ollama", db_msg);
                    persist_and_push_message(
                        current_chat_id,
                        messages,
                        message_count,
                        &chat_id,
                        "assistant",
                        db_msg,
                        visible_message_count().max(MESSAGE_WINDOW_SIZE),
                    );

                    push_toast(
                        toasts,
                        ToastKind::Warning,
                        "Model required",
                        "Choose an Ollama model in Settings before sending a message.",
                    );
                    loading_chat.set(None);
                    current_cancel.set(None);
                    return;
                }

                let mut enriched_message = user_message.clone();
                if !s.embed_model.trim().is_empty() {
                    match get_context(&user_message, &s.embed_model, 3).await {
                        Ok(context) if !context.is_empty() => {
                            rag_status.set(Some("RAG context retrieved from indexed files.".to_string()));
                            enriched_message = format!(
                                "Context from local files:\n{}\n\nUser Question: {}",
                                context, user_message
                            );
                        }
                        Ok(_) => {
                            rag_status.set(Some("No indexed context matched this message.".to_string()));
                        }
                        Err(err) => {
                            record_ui_error("rag", format!("RAG lookup failed: {err}"));
                            rag_status.set(Some(format!("RAG lookup failed: {err}")));
                        }
                    }
                } else {
                    rag_status.set(Some("RAG disabled until you choose an embedding model in Settings.".to_string()));
                }

                let mut ollama_messages = Vec::new();

                if !s.system_prompt.is_empty() {
                    ollama_messages.push(OllamaMessage {
                        role: "system".to_string(),
                        content: s.system_prompt.clone(),
                        images: None,
                    });
                }

                if let Some(active_server) = s.active_mcp_server() {
                    let mut mcp_note = format!(
                        "Active MCP integration: {} ({}).\nMCP session state: {}.",
                        active_server.name,
                        transport_label(&active_server.transport),
                        mcp_status().unwrap_or_else(|| "unknown".to_string())
                    );
                    if let Some(last_error) = mcp_last_error() {
                        mcp_note.push_str(&format!("\nLast MCP error: {last_error}"));
                    }
                    if let Some(tools) = mcp_tools_cache() {
                        mcp_note.push_str(&format!("\nLast MCP tools listing:\n{tools}"));
                    }
                    mcp_note.push_str(
                        "\nUse this note to answer MCP availability or tool-list questions accurately in normal chat. Tool execution itself still requires explicit /mcp commands."
                    );
                    ollama_messages.push(OllamaMessage {
                        role: "system".to_string(),
                        content: mcp_note,
                        images: None,
                    });
                }

                for (role, content) in history_snapshot.iter() {
                    let (prompt_content, prompt_images) = render_message_for_model(content);
                    if prompt_content.trim().is_empty() && prompt_images.is_empty() {
                        continue;
                    }
                    ollama_messages.push(OllamaMessage {
                        role: role.clone(),
                        content: prompt_content,
                        images: if prompt_images.is_empty() { None } else { Some(prompt_images) },
                    });
                }

                ollama_messages.push(OllamaMessage {
                    role: "user".to_string(),
                    content: enriched_message,
                    images: if user_images.is_empty() { None } else { Some(user_images) },
                });

                let params_json = serde_json::json!({
                    "temperature": s.temperature,
                    "top_p": s.top_p,
                    "max_tokens": s.max_tokens
                });

                let request = OllamaChatRequest {
                    model: s.model.clone(),
                    messages: ollama_messages,
                    stream: false,
                    parameters: Some(params_json),
                };

                let ollama_url = "http://localhost:11434/api/chat";

                match http_client().post(ollama_url).json(&request).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            match response.json::<OllamaChatResponse>().await {
                                Ok(api_response) => {
                                    if cancel_flag.load(Ordering::Relaxed) {
                                    } else {
                                        persist_and_push_message(
                                            current_chat_id,
                                            messages,
                                            message_count,
                                            &chat_id,
                                            "assistant",
                                            api_response.message.content,
                                            visible_message_count().max(MESSAGE_WINDOW_SIZE),
                                        );
                                    }
                                }
                                Err(e) => {
                                    record_ui_error("ollama", format!("Failed to parse Ollama response: {e}"));
                                    let err_text = "Error: Failed to parse response from Ollama";
                                    persist_and_push_message(
                                        current_chat_id,
                                        messages,
                                        message_count,
                                        &chat_id,
                                        "assistant",
                                        err_text,
                                        visible_message_count().max(MESSAGE_WINDOW_SIZE),
                                    );
                                    push_toast(
                                        toasts,
                                        ToastKind::Error,
                                        "Bad Ollama response",
                                        "RustyChat could not parse the model response.",
                                    );
                                }
                            }
                        } else {
                            record_ui_error("ollama", format!("Ollama API returned status {}", response.status()));
                            let err_text =
                                format!("Error: Ollama API returned status {}", response.status());
                            persist_and_push_message(
                                current_chat_id,
                                messages,
                                message_count,
                                &chat_id,
                                "assistant",
                                err_text,
                                visible_message_count().max(MESSAGE_WINDOW_SIZE),
                            );
                            push_toast(
                                toasts,
                                ToastKind::Error,
                                "Ollama request failed",
                                format!("The Ollama API returned status {}.", response.status()),
                            );
                        }
                    }
                    Err(e) => {
                        record_ui_error("ollama", format!("Could not connect to Ollama: {e}"));
                        let err_text = "Error: Could not connect to Ollama. Make sure Ollama is running at http://localhost:11434";
                        persist_and_push_message(
                            current_chat_id,
                            messages,
                            message_count,
                            &chat_id,
                            "assistant",
                            err_text,
                            visible_message_count().max(MESSAGE_WINDOW_SIZE),
                        );
                        push_toast(
                            toasts,
                            ToastKind::Error,
                            "Ollama unavailable",
                            "RustyChat could not connect to Ollama at http://localhost:11434.",
                        );
                    }
                }

                loading_chat.set(None);
                current_cancel.set(None);
            }
        }
    };

    let is_current_chat_loading = loading_chat()
        .as_ref()
        .map(|l| current_chat_id().as_ref().map(|c| c == l).unwrap_or(false))
        .unwrap_or(false);
    let is_other_chat_loading = loading_chat()
        .as_ref()
        .map(|l| current_chat_id().as_ref().map(|c| c != l).unwrap_or(false))
        .unwrap_or(false);
    let has_model = !settings().model.trim().is_empty();
    let total_messages = message_count();
    let hidden_message_count = total_messages.saturating_sub(visible_message_count());
    let can_send = current_chat_id().is_some()
        && has_model
        && (!input_text().trim().is_empty() || !pending_attachments().is_empty())
        && !is_other_chat_loading;
    let composer_hint = if !has_model {
        Some("Select an Ollama model in Settings before sending messages.")
    } else if current_chat_id().is_none() {
        Some("Create or select a chat to start typing.")
    } else {
        None
    };
    let selected_tool_name = selected_mcp_tool();
    let selected_tool_description = mcp_tool_entries()
        .iter()
        .find(|(name, _)| name == &selected_tool_name)
        .map(|(_, description)| description.clone());
    let selected_tool_example = mcp_tool_example(&selected_tool_name);
    let mcp_args_placeholder = match selected_tool_name.as_str() {
        "list_directory" | "list_directory_with_sizes" | "directory_tree" | "read_text_file"
        | "read_file" | "read_media_file" | "get_file_info" | "create_directory" => {
            "Paste a path. RustyChat will fill the JSON for you."
        }
        "list_allowed_directories" => "No input needed. Leave this blank.",
        "search_files" => "Use `folder path | pattern`, for example `C:\\project | **/*.rs`.",
        "read_multiple_files" => "Paste one file path per line, or enter full JSON.",
        "move_file" => "Use `source path -> destination path`, or enter full JSON.",
        "" => "Load MCP tools, then choose one.",
        _ => "Enter JSON arguments for this tool.",
    };
    let submit_message = std::rc::Rc::new(std::cell::RefCell::new({
        to_owned![
            current_chat_id,
            input_text,
            messages,
            message_count,
            current_cancel,
            loading_chat,
            send_to_ollama,
            visible_message_count,
            settings,
            rag_status,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error,
            pending_attachments,
            show_composer_tools,
            toasts
        ];
        move || {
            if let Some(chat_id) = current_chat_id() {
                let text = input_text();
                let history_snapshot = {
                    let conn = init_db();
                    load_chat_messages(&conn, &chat_id, MAX_HISTORY_MESSAGES)
                };
                let attachments = pending_attachments();

                if text.trim().is_empty() && attachments.is_empty() {
                    return;
                }

                let conn = init_db();

                let mut user_text = text.clone();
                const MAX_MESSAGE_LEN: usize = 1_000_000;
                if user_text.len() > MAX_MESSAGE_LEN {
                    user_text.truncate(MAX_MESSAGE_LEN);
                }
                let stored_user_content = serialize_message_payload(&user_text, &attachments);
                let attachment_context = build_attachment_prompt(&attachments);
                let attachment_images = build_attachment_images(&attachments);

                conn.execute(
                    "INSERT INTO messages (chat_id, role, content)
                     VALUES (?1, 'user', ?2)",
                    params![chat_id, stored_user_content.clone()],
                ).unwrap();

                enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);

                messages.push(("user".into(), stored_user_content.clone()));
                message_count.set(message_count().saturating_add(1));
                input_text.set("".to_string());
                pending_attachments.set(Vec::new());
                show_composer_tools.set(false);

                if !text.trim_start().starts_with("/mcp") {
                    if let Some(local_reply) = maybe_handle_mcp_meta_query(
                        &text,
                        &active_mcp_server_name,
                        mcp_status(),
                        mcp_tools_cache(),
                        mcp_last_error(),
                    ) {
                        persist_chat_message(&conn, &chat_id, "assistant", &local_reply);
                        refresh_visible_chat_window(
                            current_chat_id,
                            messages,
                            message_count,
                            &chat_id,
                            visible_message_count().max(MESSAGE_WINDOW_SIZE),
                        );
                        push_toast(
                            toasts,
                            ToastKind::Info,
                            "MCP status",
                            "Answered from cached MCP session state.",
                        );
                        return;
                    }
                }

                if text.trim_start().starts_with("/mcp") {
                    run_mcp_command(chat_id, text, None);
                } else {
                    let cancel_flag = Arc::new(AtomicBool::new(false));
                    current_cancel.set(Some(cancel_flag.clone()));
                    loading_chat.set(Some(chat_id.clone()));
                    let final_user_prompt = if attachment_context.is_empty() {
                        text
                    } else if text.trim().is_empty() {
                        attachment_context
                    } else {
                        format!("{attachment_context}\n\nUser Message:\n{text}")
                    };
                    spawn({
                        let chat_id = chat_id.clone();
                        let cancel_flag = cancel_flag.clone();
                        send_to_ollama(
                            chat_id,
                            history_snapshot,
                            final_user_prompt,
                            attachment_images,
                            cancel_flag,
                        )
                    });
                }
            }
        }
    }));
    let mut switch_active_mcp_server = {
        to_owned![
            settings,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error,
            mcp_tool_entries,
            selected_mcp_tool,
            rag_status
        ];
        move |server_id: String| {
            let mut next_settings = settings();
            next_settings.active_mcp_server_id = server_id;
            next_settings.mcp_server_command = next_settings
                .active_mcp_server()
                .map(|server| server.target)
                .unwrap_or_default();
            let conn = init_db();
            save_settings(&conn, &next_settings);
            settings.set(next_settings);
            mcp_status.set(None);
            mcp_tools_cache.set(None);
            mcp_last_error.set(None);
            mcp_tool_entries.set(Vec::new());
            selected_mcp_tool.set(String::new());
            rag_status.set(Some("Switched active MCP integration. Load tools for the new server.".to_string()));
        }
    };
    let submit_message_keydown = submit_message.clone();
    let submit_message_click = submit_message.clone();

    rsx! {
        div { class: "chat-window",

            div { class: "chat-header",
                div { class: "chat-header-top",
                    div {
                        h2 { "{header_title}" }
                        p { class: "model-indicator", "Model: {model_display}" }
                        p { class: "model-indicator secondary", "Embeddings: {embed_model_display}" }
                        p { class: "model-indicator secondary", "{mcp_display}" }
                        p { class: "model-indicator secondary", "{corpus_display}" }
                    }

                    div { class: "chat-header-actions",
                        if active_mcp_server_present {
                            button {
                                class: if show_mcp_workspace() { "header-workspace-btn active" } else { "header-workspace-btn" },
                                onclick: move |_| show_mcp_workspace.set(!show_mcp_workspace()),
                                if show_mcp_workspace() { "Hide MCP" } else { "Open MCP" }
                            }
                        }
                        button {
                            class: "header-workspace-btn clear-index-header-btn",
                            disabled: is_indexing(),
                            onclick: move |_| {
                                let conn = init_db();
                                let cleared = clear_document_chunks(&conn);
                                refresh_index_metrics();
                                rag_status.set(Some(format!("Cleared {cleared} indexed document chunks.")));
                                push_toast(
                                    toasts,
                                    ToastKind::Success,
                                    "Index cleared",
                                    format!("Removed {cleared} indexed document chunks."),
                                );
                            },
                            "Clear Index"
                        }
                    }
                }
            }

            div { class: "chat-messages",
                if hidden_message_count > 0 {
                    button {
                        class: "history-load-more",
                        onclick: move |_| {
                            if let Some(chat_id) = current_chat_id() {
                                let next_limit = (visible_message_count() + 120).min(total_messages);
                                let conn = init_db();
                                messages.set(load_chat_messages(&conn, &chat_id, next_limit as i64));
                                message_count.set(count_chat_messages(&conn, &chat_id) as usize);
                                visible_message_count.set(next_limit);
                            }
                        },
                        "Load {hidden_message_count} older messages"
                    }
                }
                {messages().iter().map(|(role, content)| {
                    rsx! {
                        Message {
                            role: role.clone(),
                            content: content.clone(),
                            code_execution_enabled: settings().allow_code_execution,
                            execution_timeout_secs: settings().execution_timeout_secs
                        }
                    }
                })}

                if messages().is_empty() && current_chat_id().is_some() {
                    div { class: "message assistant-message empty-state-message",
                        div { class: "empty-state-copy",
                            h3 { "Ready when you are" }
                            p { "Type a prompt below, or attach a folder to index local files before asking questions about them." }
                        }
                    }
                }

                { if is_current_chat_loading {
                    rsx! {
                        div { class: "message assistant-message loading-message",
                            p { "Thinking..." }
                            div { class: "loading-dots" }
                        }
                    }
                } else {
                    rsx!( Fragment {} )
                }}
            }

            div { class: "chat-input-area",
                div { class: "composer-tools-anchor",
                    button {
                        class: if show_composer_tools() { "composer-plus-btn active" } else { "composer-plus-btn" },
                        disabled: is_current_chat_loading || is_indexing() || mcp_is_busy,
                        onclick: move |_| show_composer_tools.set(!show_composer_tools()),
                        if show_composer_tools() { "×" } else { "+" }
                    }

                    if show_composer_tools() {
                        div { class: "composer-tools-popover",
                            if active_mcp_server_present {
                                button {
                                    class: if show_mcp_workspace() { "composer-tool-item active" } else { "composer-tool-item" },
                                    disabled: mcp_is_busy,
                                    onclick: move |_| {
                                        show_mcp_workspace.set(!show_mcp_workspace());
                                        show_composer_tools.set(false);
                                    },
                                    span { class: "composer-tool-icon", "⌘" }
                                    span { class: "composer-tool-copy",
                                        strong { if show_mcp_workspace() { "Hide MCP panel" } else { "Open MCP panel" } }
                                        small { "Tools, file browsing, MCP actions" }
                                    }
                                }
                            }

                            button {
                                class: "composer-tool-item",
                                disabled: is_current_chat_loading,
                                onclick: move |_| {
                                    if let Some(paths) = FileDialog::new().pick_files() {
                                        let mut next = pending_attachments();
                                        for path in paths {
                                            let attachment = make_attachment(&path);
                                            if !next.iter().any(|existing| existing.path == attachment.path) {
                                                next.push(attachment);
                                            }
                                        }
                                        pending_attachments.set(next);
                                    }
                                    show_composer_tools.set(false);
                                },
                                span { class: "composer-tool-icon", "⤴" }
                                span { class: "composer-tool-copy",
                                    strong { "Attach files" }
                                    small { "Share documents or images in chat" }
                                }
                            }

                            button {
                                class: "composer-tool-item",
                                disabled: is_current_chat_loading,
                                onclick: move |_| {
                                    if let Some(path) = FileDialog::new().pick_folder() {
                                        let attachment = make_attachment(&path);
                                        let mut next = pending_attachments();
                                        if !next.iter().any(|existing| existing.path == attachment.path) {
                                            next.push(attachment);
                                        }
                                        pending_attachments.set(next);
                                    }
                                    show_composer_tools.set(false);
                                },
                                span { class: "composer-tool-icon", "⬚" }
                                span { class: "composer-tool-copy",
                                    strong { "Attach folder" }
                                    small { "Share a folder directly in chat" }
                                }
                            }

                            button {
                                class: "composer-tool-item",
                                disabled: is_indexing(),
                                onclick: move |_| {
                                    if let Some(path) = FileDialog::new().pick_folder() {
                                        let path_str = path.to_string_lossy().to_string();
                                        let embed_model = settings().embed_model.clone();
                                        if embed_model.trim().is_empty() {
                                            rag_status.set(Some("Choose an embedding model in Settings before indexing a folder.".to_string()));
                                            push_toast(
                                                toasts,
                                                ToastKind::Warning,
                                                "Embedding model required",
                                                "Choose an embedding model in Settings before indexing a folder.",
                                            );
                                            show_composer_tools.set(false);
                                            return;
                                        }
                                        is_indexing.set(true);
                                        rag_status.set(Some(format!("Indexing folder: {}", path_str)));
                                        let mut indexed_files = indexed_files.clone();
                                        let mut indexed_chunks = indexed_chunks.clone();
                                        let mut rag_status = rag_status.clone();
                                        let mut is_indexing = is_indexing.clone();
                                        let toasts = toasts.clone();
                                        spawn(async move {
                                            let status = match index_directory(&path_str, &embed_model).await {
                                                Ok(stats) => {
                                                    let conn = init_db();
                                                    let total_files = count_indexed_files(&conn);
                                                    let total_chunks = count_document_chunks(&conn);
                                                    indexed_files.set(total_files);
                                                    indexed_chunks.set(total_chunks);
                                                    if stats.files_indexed == 0 || stats.chunks_indexed == 0 {
                                                        push_toast(
                                                            toasts,
                                                            ToastKind::Warning,
                                                            "Nothing indexed",
                                                            "The selected folder did not contain supported text files.",
                                                        );
                                                        format!(
                                                            "No supported text files were indexed from {}. Supported types: .rs, .md, .txt, .py, .js, .ts, .toml, .json, .c, .cpp, .h",
                                                            path_str
                                                        )
                                                    } else {
                                                        push_toast(
                                                            toasts,
                                                            ToastKind::Success,
                                                            "Folder indexed",
                                                            format!(
                                                                "Added {} files and {} chunks to the local index.",
                                                                stats.files_indexed, stats.chunks_indexed
                                                            ),
                                                        );
                                                        format!(
                                                            "Indexed {} files and {} chunks from {} (replaced {} old chunks). Corpus now has {} files and {} chunks.",
                                                            stats.files_indexed,
                                                            stats.chunks_indexed,
                                                            path_str,
                                                            stats.chunks_replaced,
                                                            total_files,
                                                            total_chunks
                                                        )
                                                    }
                                                }
                                                Err(err) => {
                                                    record_ui_error("rag", format!("Indexing failed: {err}"));
                                                    push_toast(
                                                        toasts,
                                                        ToastKind::Error,
                                                        "Indexing failed",
                                                        err.to_string(),
                                                    );
                                                    format!("Indexing failed: {}", err)
                                                },
                                            };
                                            is_indexing.set(false);
                                            rag_status.set(Some(status));
                                        });
                                    }
                                    show_composer_tools.set(false);
                                },
                                span { class: "composer-tool-icon", "◫" }
                                span { class: "composer-tool-copy",
                                    strong { if is_indexing() { "Indexing folder..." } else { "Index folder for RAG" } }
                                    small { "Add a project or docs folder to local retrieval" }
                                }
                            }
                        }
                    }
                }
                if !pending_attachments().is_empty() {
                    div { class: "input-attachments-row",
                        {pending_attachments().iter().enumerate().map(|(idx, attachment)| {
                            let attachment_name = attachment.name.clone();
                            let attachment_kind = attachment.kind.clone();
                            let attachment_path = attachment.path.clone();
                            rsx!(
                                div { class: "input-attachment-chip",
                                    if attachment_kind == AttachmentKind::Image {
                                        img {
                                            class: "input-attachment-preview",
                                            src: "{attachment_image_src(&attachment_path)}",
                                            alt: "{attachment_name}",
                                        }
                                    } else if attachment_kind == AttachmentKind::Folder {
                                        div { class: "input-attachment-file", "DIR" }
                                    } else {
                                        div { class: "input-attachment-file", "FILE" }
                                    }
                                    div { class: "input-attachment-name", "{attachment_name}" }
                                    button {
                                        class: "input-attachment-remove",
                                        onclick: move |_| {
                                            let mut next = pending_attachments();
                                            if idx < next.len() {
                                                next.remove(idx);
                                                pending_attachments.set(next);
                                            }
                                        },
                                        "×"
                                    }
                                }
                            )
                        })}
                    }
                }
                textarea {
                    class: "chat-input",
                    placeholder: "Send a message...",
                    value: "{input_text}",
                    oninput: move |e| input_text.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter
                            && !e.modifiers().contains(Modifiers::SHIFT)
                            && !e.is_auto_repeating()
                        {
                            e.prevent_default();
                            if can_send {
                                (submit_message_keydown.borrow_mut())();
                            }
                        }
                    },
                    disabled: is_current_chat_loading,
                }

                button {
                    class: if is_current_chat_loading { "send-button big stop-mode" } else { "send-button big" },
                    disabled: if is_current_chat_loading { false } else { !can_send },
                    onclick: move |_| {
                        if is_current_chat_loading {
                            if let Some(cancel) = current_cancel() {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            loading_chat.set(None);
                            current_cancel.set(None);
                        } else {
                            (submit_message_click.borrow_mut())();
                        }
                    },
                    if is_current_chat_loading { "■" } else { "➤" }
                }
            }

            if active_mcp_server_present && show_mcp_workspace() {
                div {
                    class: "mcp-workspace-backdrop",
                    onclick: move |_| show_mcp_workspace.set(false),
                }
                aside { class: "mcp-workspace-drawer",
                    div { class: "mcp-workspace-shell",
                        div { class: "mcp-workspace-header",
                            div {
                                span { class: "mcp-workspace-kicker", "Tool Workspace" }
                                h3 { "MCP Control Desk" }
                                p { "Load tools once, then run filesystem actions with simpler inputs instead of slash commands and raw JSON." }
                            }
                            button {
                                class: "mcp-close-btn",
                                onclick: move |_| show_mcp_workspace.set(false),
                                "×"
                            }
                        }

                        div { class: "mcp-status-card",
                            span { class: "mcp-status-pill", "{mcp_display}" }
                            if settings().mcp_servers.len() > 1 {
                                div { class: "mcp-active-switcher",
                                    label { class: "mcp-field-label", "Active integration" }
                                    select {
                                        class: "input mcp-tool-select",
                                        value: "{settings().active_mcp_server_id}",
                                        disabled: mcp_is_busy,
                                        onchange: move |e| switch_active_mcp_server(e.value()),
                                        {settings().mcp_servers.iter().map(|server| rsx!(
                                            option { value: "{server.id}", selected: server.id == settings().active_mcp_server_id, "{server.name}" }
                                        ))}
                                    }
                                }
                            }
                            if let Some(err) = mcp_last_error() {
                                p { class: "mcp-status-copy error", "{err}" }
                            } else if let Some(status) = rag_status() {
                                p { class: "mcp-status-copy", "{status}" }
                            } else {
                                p { class: "mcp-status-copy", "Use this workspace to load tools, inspect files, and run MCP actions without remembering command syntax." }
                            }
                            if let Some(server) = active_mcp_server.as_ref() {
                                p { class: "mcp-status-copy", "Transport: {transport_label(&server.transport)}" }
                                p { class: "mcp-status-copy", "Target: {server.target}" }
                            }
                        }

                        div { class: "mcp-toolbar",
                            button {
                                class: "mcp-primary-btn",
                                disabled: current_chat_id().is_none() || mcp_is_busy,
                                onclick: move |_| {
                                    if let Some(chat_id) = current_chat_id() {
                                        run_mcp_command(
                                            chat_id,
                                            "/mcp tools".to_string(),
                                            Some("Load MCP tools".to_string()),
                                        );
                                    }
                                },
                                if mcp_is_busy { "Working..." } else { "Load Tools" }
                            }
                            div { class: "mcp-quick-actions",
                                button {
                                    class: "mcp-chip-btn",
                                    disabled: mcp_is_busy,
                                    onclick: move |_| selected_mcp_tool.set("list_directory".to_string()),
                                    "Browse Folder"
                                }
                                button {
                                    class: "mcp-chip-btn",
                                    disabled: mcp_is_busy,
                                    onclick: move |_| selected_mcp_tool.set("directory_tree".to_string()),
                                    "Show Tree"
                                }
                                button {
                                    class: "mcp-chip-btn",
                                    disabled: mcp_is_busy,
                                    onclick: move |_| selected_mcp_tool.set("read_text_file".to_string()),
                                    "Read File"
                                }
                                button {
                                    class: "mcp-chip-btn",
                                    disabled: mcp_is_busy,
                                    onclick: move |_| selected_mcp_tool.set("search_files".to_string()),
                                    "Search Files"
                                }
                            }
                        }

                        if !mcp_tool_entries().is_empty() {
                            div { class: "mcp-workspace-form",
                                label { class: "mcp-field-label", "Tool" }
                                select {
                                    class: "input mcp-tool-select",
                                    value: "{selected_mcp_tool}",
                                    onchange: move |e| selected_mcp_tool.set(e.value()),
                                    {mcp_tool_entries().iter().map(|(name, _)| rsx!(
                                        option { value: "{name}", selected: name == &selected_mcp_tool(), "{name}" }
                                    ))}
                                }

                                label { class: "mcp-field-label", "Input" }
                                textarea {
                                    class: "textarea mcp-tool-args fancy",
                                    value: "{mcp_tool_args}",
                                    placeholder: "{mcp_args_placeholder}",
                                    oninput: move |e| mcp_tool_args.set(e.value()),
                                    disabled: mcp_is_busy,
                                }

                                button {
                                    class: "mcp-primary-btn run",
                                    disabled: current_chat_id().is_none() || selected_mcp_tool().trim().is_empty() || mcp_is_busy,
                                    onclick: move |_| {
                                        if let Some(chat_id) = current_chat_id() {
                                            let tool = selected_mcp_tool();
                                            match build_friendly_mcp_command(&tool, &mcp_tool_args()) {
                                                Ok(command) => {
                                                    let summary = if mcp_tool_args().trim().is_empty() {
                                                        format!("Run MCP tool: {tool}")
                                                    } else {
                                                        format!("Run MCP tool: {tool} {}", mcp_tool_args().trim())
                                                    };
                                                    run_mcp_command(chat_id, command, Some(summary));
                                                }
                                                Err(err) => {
                                                    rag_status.set(Some(err));
                                                }
                                            }
                                        }
                                    },
                                    "Run Tool"
                                }
                            }

                            if let Some(description) = selected_tool_description {
                                div { class: "mcp-tool-description-card",
                                    span { class: "mcp-field-label", "What this tool does" }
                                    p { "{description}" }
                                    div { class: "mcp-example-block",
                                        span { class: "mcp-field-label", "Quick format" }
                                        pre { "{selected_tool_example}" }
                                    }
                                }
                            }
                        } else {
                            div { class: "mcp-empty-state-card",
                                h4 { "No tools loaded yet" }
                                p { "Click `Load Tools` to fetch the server's available actions. Once loaded, this panel will give you tool selection and simpler inputs." }
                            }
                        }
                    }
                }
            }

            if let Some(hint) = composer_hint {
                div { class: "composer-hint", "{hint}" }
            }

            if let Some(status) = rag_status() {
                div { class: "composer-hint rag-status", "{status}" }
            }
        }
    }
}

/* ================= MESSAGE ================= */

#[component]
pub fn Message(
    role: String,
    content: String,
    code_execution_enabled: bool,
    execution_timeout_secs: i32,
) -> Element {
    let (attachments, body_content) = parse_message_payload(&content);
    let class_name = if role == "user" {
        "message user-message"
    } else {
        "message assistant-message"
    };
    let parsed_mcp_tools = if role == "assistant" {
        parse_mcp_tools_message(&body_content)
    } else {
        None
    };
    let parsed_mcp_files = if role == "assistant" {
        parse_mcp_file_rows(&body_content)
    } else {
        None
    };
    let parsed_mcp_info = if role == "assistant" {
        parse_mcp_info_rows(&body_content)
    } else {
        None
    };
    let mcp_error_text = if role == "assistant" && body_content.trim().starts_with("MCP Error:") {
        Some(body_content.trim().trim_start_matches("MCP Error:").trim().to_string())
    } else {
        None
    };

    if body_content.contains("<think>") && body_content.contains("</think>") {
        let think_start = body_content.find("<think>").unwrap() + "<think>".len();
        let think_end = body_content.find("</think>").unwrap();
        let think_content = &body_content[think_start..think_end].trim();

        let before_think = &body_content[..think_start - "<think>".len()];
        let after_think = &body_content[think_end + "</think>".len()..];

        rsx! {
            div { class: "{class_name}",
                if !attachments.is_empty() {
                    AttachmentGallery { attachments: attachments.clone() }
                }
                {if !before_think.is_empty() {
                    rsx! { Markdown {
                        content: before_think.to_string(),
                        code_execution_enabled,
                        execution_timeout_secs,
                    } }
                } else {
                    rsx! { Fragment {} }
                }}

                div { class: "think-bubble",
                    p { class: "think-label", "🤔 Thinking..." }
                    div { class: "think-content dim-text",
                        "\n"
                        "{think_content}"
                        "\n"
                    }
                }

                {if !after_think.is_empty() {
                    rsx! { Markdown {
                        content: after_think.to_string(),
                        code_execution_enabled,
                        execution_timeout_secs,
                    } }
                } else {
                    rsx! { Fragment {} }
                }}
            }
        }
    } else {
        rsx! {
            div { class: "{class_name}",
                if !attachments.is_empty() {
                    AttachmentGallery { attachments: attachments.clone() }
                }
                if !body_content.trim().is_empty() {
                    if let Some(error_text) = mcp_error_text {
                        McpErrorCard { message: error_text }
                    } else if let Some(tools) = parsed_mcp_tools {
                        McpToolsCard { tools }
                    } else if let Some(rows) = parsed_mcp_files {
                        McpFileListCard { rows }
                    } else if let Some(rows) = parsed_mcp_info {
                        McpInfoCard { rows }
                    } else {
                        Markdown {
                            content: body_content.clone(),
                            code_execution_enabled,
                            execution_timeout_secs,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn McpErrorCard(message: String) -> Element {
    rsx! {
        div { class: "mcp-result-card error",
            span { class: "mcp-result-kicker", "MCP Error" }
            p { class: "mcp-result-copy", "{message}" }
        }
    }
}

#[component]
fn McpToolsCard(tools: Vec<(String, String)>) -> Element {
    rsx! {
        div { class: "mcp-result-card",
            span { class: "mcp-result-kicker", "Available Tools" }
            div { class: "mcp-tool-grid",
                {tools.iter().map(|(name, description)| rsx!(
                    div { class: "mcp-tool-chip-card",
                        strong { "{name}" }
                        if !description.is_empty() {
                            p { "{description}" }
                        }
                    }
                ))}
            }
        }
    }
}

#[component]
fn McpFileListCard(rows: Vec<(String, String)>) -> Element {
    rsx! {
        div { class: "mcp-result-card",
            span { class: "mcp-result-kicker", "Filesystem Result" }
            div { class: "mcp-file-list",
                {rows.iter().map(|(kind, name)| rsx!(
                    div { class: "mcp-file-row",
                        span { class: "mcp-file-kind", "{kind}" }
                        span { class: "mcp-file-name", "{name}" }
                    }
                ))}
            }
        }
    }
}

#[component]
fn McpInfoCard(rows: Vec<(String, String)>) -> Element {
    rsx! {
        div { class: "mcp-result-card",
            span { class: "mcp-result-kicker", "Details" }
            div { class: "mcp-info-list",
                {rows.iter().map(|(key, value)| rsx!(
                    div { class: "mcp-info-row",
                        span { class: "mcp-info-key", "{key}" }
                        span { class: "mcp-info-value", "{value}" }
                    }
                ))}
            }
        }
    }
}

#[component]
pub fn ToastHost(toasts: Signal<Vec<ToastNotification>>) -> Element {
    rsx! {
        div { class: "toast-host",
            {toasts().iter().map(|toast| {
                let toast_id = toast.id.clone();
                let toast_kind = match toast.kind {
                    ToastKind::Info => "info",
                    ToastKind::Success => "success",
                    ToastKind::Warning => "warning",
                    ToastKind::Error => "error",
                };
                let title = toast.title.clone();
                let message = toast.message.clone();
                rsx!(
                    div { class: "toast-card toast-{toast_kind}",
                        div { class: "toast-copy",
                            strong { "{title}" }
                            p { "{message}" }
                        }
                        button {
                            class: "toast-dismiss",
                            onclick: move |_| toasts.retain(|item| item.id != toast_id),
                            "×"
                        }
                    }
                )
            })}
        }
    }
}

#[component]
fn AttachmentGallery(attachments: Vec<ChatAttachment>) -> Element {
    rsx! {
        div { class: "message-attachments",
            {attachments.iter().map(|attachment| {
                let name = attachment.name.clone();
                let path = attachment.path.clone();
                let kind = attachment.kind.clone();
                rsx!(
                    div { class: "message-attachment-card",
                        if kind == AttachmentKind::Image {
                            img {
                                class: "message-attachment-image",
                                src: "{attachment_image_src(&path)}",
                                alt: "{name}",
                            }
                        } else if kind == AttachmentKind::Folder {
                            div { class: "message-attachment-file-badge", "DIR" }
                        } else {
                            div { class: "message-attachment-file-badge", "FILE" }
                        }
                        div { class: "message-attachment-copy",
                            div { class: "message-attachment-name", "{name}" }
                            div { class: "message-attachment-path", "{path}" }
                        }
                    }
                )
            })}
        }
    }
}

fn maybe_handle_mcp_meta_query(
    user_text: &str,
    active_mcp_server_name: &str,
    mcp_status: Option<String>,
    mcp_tools_cache: Option<String>,
    mcp_last_error: Option<String>,
) -> Option<String> {
    let lower = user_text.trim().to_lowercase();
    let normalized = lower
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>();
    let contains_phrase = |phrase: &str| normalized.contains(phrase);

    let asks_tools = contains_phrase("what mcp tools")
        || contains_phrase("which mcp tools")
        || contains_phrase("list mcp tools")
        || contains_phrase("show mcp tools")
        || contains_phrase("what tools are available")
        || contains_phrase("which tools are available")
        || contains_phrase("available mcp tools");
    let asks_status = contains_phrase("is the mcp server running")
        || contains_phrase("is mcp running")
        || contains_phrase("is the mcp server working")
        || contains_phrase("is mcp working")
        || contains_phrase("mcp server status")
        || contains_phrase("mcp status")
        || contains_phrase("is mcp connected")
        || contains_phrase("is the mcp server connected");

    if !asks_tools && !asks_status {
        return None;
    }

    if asks_tools {
        return Some(match mcp_tools_cache {
            Some(tools) => format!("The last MCP tools listing for this session was:\n\n{tools}"),
            None => {
                if active_mcp_server_name.trim().is_empty() {
                    "No MCP server is configured yet, so there is no tool list available.".to_string()
                } else {
                    format!("I don't have a cached MCP tool list yet for `{active_mcp_server_name}`. Run `/mcp tools` first, then I can answer normal questions about the available MCP tools.")
                }
            }
        });
    }

    if asks_status {
        return Some(match mcp_status.as_deref() {
            Some("ready") => {
                let mut msg = "Yes. Based on this session state, the MCP server is running and responded successfully the last time it was used.".to_string();
                if let Some(tools) = mcp_tools_cache {
                    msg.push_str(&format!("\n\nCached MCP tools:\n{tools}"));
                }
                msg
            }
            Some("starting server") | Some("running command") => {
                "The MCP server is still in progress right now. The current session state shows that it is starting or running a command.".to_string()
            }
            Some("error") => {
                if let Some(err) = mcp_last_error {
                    format!("No. The latest MCP attempt failed with this error:\n\n{err}")
                } else {
                    "No. The latest MCP attempt failed, but there is no cached error text.".to_string()
                }
            }
            _ => {
                if active_mcp_server_name.trim().is_empty() {
                    "No MCP server is configured yet in Settings.".to_string()
                } else {
                    format!("I don't have a confirmed MCP status yet for `{active_mcp_server_name}`. Run `/mcp tools` once, then I can answer normal MCP status questions from the cached result.")
                }
            }
        });
    }

    None
}
