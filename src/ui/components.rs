use dioxus::prelude::*;
use reqwest::Client;
use rusqlite::params;
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
    clear_document_chunks, count_document_chunks, count_indexed_files, enforce_history_limit,
    init_db, save_settings, Settings, clamp_to_i32,
};
use crate::mcp::handle_mcp_command;
use crate::ollama::{OllamaChatRequest, OllamaChatResponse, OllamaMessage};
use crate::ui::Markdown;
use crate::rag::{index_directory, get_context};

const MAX_HISTORY_MESSAGES: i64 = 10000;
const MAX_TITLE_LEN: usize = 255;
const ATTACHMENTS_PREFIX: &str = "<rustychat-attachments>";
const ATTACHMENTS_SUFFIX: &str = "</rustychat-attachments>";

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

/* ================= SETTINGS MODAL ================= */

#[component]
pub fn SettingsModal(
    settings: Signal<Settings>,
    show_settings: Signal<bool>,
    chats: Signal<Vec<(String, String)>>,
    messages: Signal<Vec<(String, String)>>,
    current_chat_id: Signal<Option<String>>,
) -> Element {
    // local editable copies using signals
    let mut local_model = use_signal(|| settings().model.clone());
    let mut local_embed_model = use_signal(|| settings().embed_model.clone());
    let mut local_mcp_server_command = use_signal(|| settings().mcp_server_command.clone());
    let mut local_system = use_signal(|| settings().system_prompt.clone());
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
        let mut local_mcp_server_command_sig = local_mcp_server_command.clone();
        let mut local_system_sig = local_system.clone();
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
                local_mcp_server_command_sig.set(s.mcp_server_command.clone());
                local_system_sig.set(s.system_prompt.clone());
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
            local_mcp_server_command,
            local_system,
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
            let mcp_server_command_str = local_mcp_server_command().trim().to_string();

            let new_settings = Settings {
                model: model_str,
                embed_model: embed_model_str,
                mcp_server_command: mcp_server_command_str,
                system_prompt: local_system().clone(),
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
        }
    };

    let delete_all = {
        to_owned![chats, messages, current_chat_id, show_settings];
        move |_| {
            let conn = init_db();
            conn.execute("DELETE FROM messages", []).ok();
            conn.execute("DELETE FROM chats", []).ok();

            chats.set(vec![]);
            messages.set(vec![]);
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

                label { "MCP server command (optional)" }
                input {
                    class: "input",
                    value: "{local_mcp_server_command}",
                    placeholder: "Folder path, MCP HTTP URL, or full MCP server command",
                    oninput: move |e| local_mcp_server_command.set(e.value()),
                }
                p { class: "dim-text warning-text", "You can paste a folder path for the filesystem MCP server, an MCP HTTP endpoint URL, or a full stdio MCP command. Then use `/mcp tools` or `/mcp call <tool> {{json}}` in chat." }

                label { "System prompt (optional)" }
                textarea {
                    class: "textarea",
                    value: "{local_system}",
                    oninput: move |e| local_system.set(e.value()),
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
                    button { onclick: apply, "Apply" }
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
                                    let mut stmt = conn.prepare(
                                        "SELECT role, content FROM messages
                                         WHERE chat_id = ? ORDER BY id DESC LIMIT ?"
                                    ).unwrap();

                                    let rows = stmt
                                        .query_map(params![&id_for_open, MAX_HISTORY_MESSAGES], |row| {
                                            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                                        })
                                        .unwrap();

                                    let mut collected: Vec<(String, String)> = rows.map(|r| r.unwrap()).collect();
                                    collected.reverse();
                                    messages_handle.set(collected);
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
                    href: "https://github.com/KPCOFGS/RustyChat",
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
    settings: Signal<Settings>,
    chats: Signal<Vec<(String, String)>>,
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
    let mcp_display = if settings().mcp_server_command.trim().is_empty() {
        "MCP: not configured".to_string()
    } else if let Some(status) = mcp_status() {
        format!("MCP: {status}")
    } else {
        "MCP: ready".to_string()
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
            rag_status,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error,
            mcp_tool_entries,
            selected_mcp_tool
        ];
        move |chat_id: String, command_text: String, user_echo: Option<String>| {
            if let Some(display_text) = user_echo {
                let conn = init_db();
                let _ = conn.execute(
                    "INSERT INTO messages (chat_id, role, content)
                     VALUES (?1, 'user', ?2)",
                    params![chat_id, display_text.clone()],
                );
                enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);
                if current_chat_id()
                    .as_ref()
                    .map(|c| c == &chat_id)
                    .unwrap_or(false)
                {
                    messages.push(("user".into(), display_text));
                }
            }

            mcp_status.set(Some("starting server".to_string()));
            mcp_last_error.set(None);
            rag_status.set(Some("MCP: starting server...".to_string()));

            spawn(async move {
                mcp_status.set(Some("running command".to_string()));
                rag_status.set(Some("MCP: running command...".to_string()));
                let mcp_command = crate::db::load_settings(&init_db()).mcp_server_command;
                let result = handle_mcp_command(&mcp_command, &command_text).await;
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
                        output
                    }
                    Err(err) => {
                        mcp_status.set(Some("error".to_string()));
                        mcp_last_error.set(Some(err.clone()));
                        rag_status.set(Some(format!("MCP command failed: {err}")));
                        format!("MCP Error: {err}")
                    }
                };

                let _ = conn.execute(
                    "INSERT INTO messages (chat_id, role, content)
                     VALUES (?1, 'assistant', ?2)",
                    params![chat_id, assistant_text],
                );
                enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);
                if current_chat_id()
                    .as_ref()
                    .map(|c| c == &chat_id)
                    .unwrap_or(false)
                {
                    messages.push(("assistant".into(), assistant_text));
                }
            });
        }
    };

    let send_to_ollama = {
        to_owned![
            messages,
            http_client,
            loading_chat,
            current_cancel,
            current_chat_id,
            settings,
            rag_status,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error
        ];
        move |chat_id: String,
              history_snapshot: Vec<(String, String)>,
              user_message: String,
              user_images: Vec<String>,
              cancel_flag: Arc<AtomicBool>| {
            async move {
                let s = settings();
                if s.model.trim().is_empty() {
                    let conn = init_db();
                    let db_msg = "Error: No model selected. Please open Settings and choose a model before sending messages.";
                    conn.execute(
                        "INSERT INTO messages (chat_id, role, content) VALUES (?1, 'assistant', ?2)",
                        params![chat_id, db_msg],
                    ).ok();
                    enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);

                    if current_chat_id()
                        .as_ref()
                        .map(|c| c == &chat_id)
                        .unwrap_or(false)
                    {
                        messages.push(("assistant".into(), db_msg.to_string()));
                    }

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

                if !s.mcp_server_command.trim().is_empty() {
                    let mut mcp_note = format!(
                        "MCP session state: {}.",
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
                                        let conn = init_db();
                                        let _ = conn.execute(
                                            "INSERT INTO messages (chat_id, role, content)
                                             VALUES (?1, 'assistant', ?2)",
                                            params![chat_id, api_response.message.content],
                                        );
                                        enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);

                                        if current_chat_id()
                                            .as_ref()
                                            .map(|c| c == &chat_id)
                                            .unwrap_or(false)
                                        {
                                            messages.push((
                                                "assistant".into(),
                                                api_response.message.content,
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse Ollama response: {}", e);
                                    let err_text = "Error: Failed to parse response from Ollama";
                                    let conn = init_db();
                                    let _ = conn.execute(
                                        "INSERT INTO messages (chat_id, role, content) VALUES (?1, 'assistant', ?2)",
                                        params![chat_id, err_text],
                                    );
                                    enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);

                                    if current_chat_id()
                                        .as_ref()
                                        .map(|c| c == &chat_id)
                                        .unwrap_or(false)
                                    {
                                        messages.push(("assistant".into(), err_text.to_string()));
                                    }
                                }
                            }
                        } else {
                            eprintln!("Ollama API error: {}", response.status());
                            let err_text =
                                format!("Error: Ollama API returned status {}", response.status());
                            let conn = init_db();
                            let _ = conn.execute(
                                "INSERT INTO messages (chat_id, role, content) VALUES (?1, 'assistant', ?2)",
                                params![chat_id, err_text],
                            );
                            enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);

                            if current_chat_id()
                                .as_ref()
                                .map(|c| c == &chat_id)
                                .unwrap_or(false)
                            {
                                messages.push((
                                    "assistant".into(),
                                    format!(
                                        "Error: Ollama API returned status {}",
                                        response.status()
                                    ),
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to send request to Ollama: {}", e);
                        let err_text = "Error: Could not connect to Ollama. Make sure Ollama is running at http://localhost:11434";
                        let conn = init_db();
                        let _ = conn.execute(
                            "INSERT INTO messages (chat_id, role, content) VALUES (?1, 'assistant', ?2)",
                            params![chat_id, err_text],
                        );
                        enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);

                        if current_chat_id()
                            .as_ref()
                            .map(|c| c == &chat_id)
                            .unwrap_or(false)
                        {
                            messages.push(("assistant".into(), err_text.to_string()));
                        }
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
    let mut submit_message = {
        to_owned![
            current_chat_id,
            input_text,
            messages,
            current_cancel,
            loading_chat,
            send_to_ollama,
            settings,
            rag_status,
            mcp_status,
            mcp_tools_cache,
            mcp_last_error,
            pending_attachments
        ];
        move || {
            if let Some(chat_id) = current_chat_id() {
                let text = input_text();
                let history_snapshot = messages();
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
                input_text.set("".to_string());
                pending_attachments.set(Vec::new());
                show_composer_tools.set(false);

                if !text.trim_start().starts_with("/mcp") {
                    if let Some(local_reply) = maybe_handle_mcp_meta_query(
                        &text,
                        &settings().mcp_server_command,
                        mcp_status(),
                        mcp_tools_cache(),
                        mcp_last_error(),
                    ) {
                        let _ = conn.execute(
                            "INSERT INTO messages (chat_id, role, content)
                             VALUES (?1, 'assistant', ?2)",
                            params![chat_id, local_reply],
                        );
                        enforce_history_limit(&conn, &chat_id, MAX_HISTORY_MESSAGES);
                        messages.push(("assistant".into(), local_reply));
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
    };

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
                        if !settings().mcp_server_command.trim().is_empty() {
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
                            },
                            "Clear Index"
                        }
                    }
                }
            }

            div { class: "chat-messages",
                {messages().iter().map(|(role, content)| {
                    rsx! {
                        Message {
                            role: role.clone(),
                            content: content.clone()
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
                            if !settings().mcp_server_command.trim().is_empty() {
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
                                disabled: is_indexing(),
                                onclick: move |_| {
                                    if let Some(path) = FileDialog::new().pick_folder() {
                                        let path_str = path.to_string_lossy().to_string();
                                        let embed_model = settings().embed_model.clone();
                                        if embed_model.trim().is_empty() {
                                            rag_status.set(Some("Choose an embedding model in Settings before indexing a folder.".to_string()));
                                            show_composer_tools.set(false);
                                            return;
                                        }
                                        is_indexing.set(true);
                                        rag_status.set(Some(format!("Indexing folder: {}", path_str)));
                                        let mut indexed_files = indexed_files.clone();
                                        let mut indexed_chunks = indexed_chunks.clone();
                                        let mut rag_status = rag_status.clone();
                                        let mut is_indexing = is_indexing.clone();
                                        spawn(async move {
                                            let status = match index_directory(&path_str, &embed_model).await {
                                                Ok(stats) => {
                                                    let conn = init_db();
                                                    let total_files = count_indexed_files(&conn);
                                                    let total_chunks = count_document_chunks(&conn);
                                                    indexed_files.set(total_files);
                                                    indexed_chunks.set(total_chunks);
                                                    if stats.files_indexed == 0 || stats.chunks_indexed == 0 {
                                                        format!(
                                                            "No supported text files were indexed from {}. Supported types: .rs, .md, .txt, .py, .js, .ts, .toml, .json, .c, .cpp, .h",
                                                            path_str
                                                        )
                                                    } else {
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
                                                Err(err) => format!("Indexing failed: {}", err),
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
                                submit_message();
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
                            submit_message();
                        }
                    },
                    if is_current_chat_loading { "■" } else { "➤" }
                }
            }

            if !settings().mcp_server_command.trim().is_empty() && show_mcp_workspace() {
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
                            if let Some(err) = mcp_last_error() {
                                p { class: "mcp-status-copy error", "{err}" }
                            } else if let Some(status) = rag_status() {
                                p { class: "mcp-status-copy", "{status}" }
                            } else {
                                p { class: "mcp-status-copy", "Use this workspace to load tools, inspect files, and run MCP actions without remembering command syntax." }
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
pub fn Message(role: String, content: String) -> Element {
    let (attachments, body_content) = parse_message_payload(&content);
    let class_name = if role == "user" {
        "message user-message"
    } else {
        "message assistant-message"
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
                    rsx! { Markdown { content: before_think.to_string() } }
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
                    rsx! { Markdown { content: after_think.to_string() } }
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
                    Markdown { content: body_content.clone() }
                }
            }
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
    mcp_server_command: &str,
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
                if mcp_server_command.trim().is_empty() {
                    "No MCP server is configured yet, so there is no tool list available.".to_string()
                } else {
                    "I don't have a cached MCP tool list yet in this session. Run `/mcp tools` first, then I can answer normal questions about the available MCP tools.".to_string()
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
                if mcp_server_command.trim().is_empty() {
                    "No MCP server is configured yet in Settings.".to_string()
                } else {
                    "I don't have a confirmed MCP status yet for this session. Run `/mcp tools` once, then I can answer normal MCP status questions from the cached result.".to_string()
                }
            }
        });
    }

    None
}
