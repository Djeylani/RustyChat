#![allow(non_snake_case)]

use dioxus::prelude::*;
use dioxus::desktop::{Config, WindowBuilder, LogicalSize};
use rusqlite::params;
use uuid::Uuid;
use crate::db::{init_db, load_settings};
use crate::ui::{Sidebar, ChatWindow, SettingsModal, CSS};

mod db;
mod ollama;
mod ui;
mod rag;

const FAVICON: Asset = asset!("/assets/favicon.ico");

fn main() {
    // Explicitly configure the window using WindowBuilder
    let window = WindowBuilder::new()
        .with_title("RustyChat")
        .with_inner_size(LogicalSize::new(1280.0, 850.0))
        .with_resizable(true)
        .with_always_on_top(false);

    LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}

/* ================= APP ================= */

#[component]
fn App() -> Element {
    let conn = init_db();

    let chats = use_signal(|| Vec::<(String, String)>::new());
    let current_chat_id = use_signal(|| Option::<String>::None);
    let messages = use_signal(|| Vec::<(String, String)>::new());

    // settings and modal visibility
    let settings = use_signal(|| load_settings(&conn));
    let show_settings = use_signal(|| false);

    // load chats once
    {
        let mut chats = chats.clone();
        let mut current_chat_id = current_chat_id.clone();
        let mut messages = messages.clone();
        use_effect(move || {
            let conn = init_db();
            let mut loaded_chats: Vec<(String, String)> = {
                let mut stmt = conn
                    .prepare("SELECT id, title FROM chats ORDER BY rowid DESC")
                    .unwrap();
                let rows = stmt
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .unwrap();

                rows.map(|r| r.unwrap()).collect()
            };

            if loaded_chats.is_empty() {
                let new_id = Uuid::new_v4().to_string();
                let title = "New Chat".to_string();

                conn.execute(
                    "INSERT INTO chats (id, title) VALUES (?1, ?2)",
                    params![new_id, title],
                )
                .unwrap();

                loaded_chats.push((new_id, title));
            }

            let selected_chat_id = loaded_chats.first().map(|(id, _)| id.clone());
            let selected_messages = if let Some(chat_id) = selected_chat_id.as_ref() {
                let mut stmt = conn
                    .prepare(
                        "SELECT role, content FROM messages
                         WHERE chat_id = ?1 ORDER BY id DESC LIMIT 10000",
                    )
                    .unwrap();
                let rows = stmt
                    .query_map(params![chat_id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .unwrap();

                let mut collected: Vec<(String, String)> = rows.map(|r| r.unwrap()).collect();
                collected.reverse();
                collected
            } else {
                Vec::new()
            };

            chats.set(loaded_chats);
            current_chat_id.set(selected_chat_id);
            messages.set(selected_messages);
        });
    }

    // Use flex percentages instead of hardcoded 100vw/vh to allow window resizing
    let container_style = "width: 100%; height: 100%;".to_string();

    // apply zoom using CSS 'zoom'
    let zoom_style = format!("zoom: {}%;", settings().zoom);

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        style { "{CSS}" }

        div { class: "outer-wrapper", style: "{container_style}",
            div { class: "app-container", style: "{zoom_style}",
                Sidebar {
                    chats: chats.clone(),
                    current_chat_id: current_chat_id.clone(),
                    messages: messages.clone(),
                    show_settings: show_settings.clone()
                }
                ChatWindow {
                    current_chat_id: current_chat_id.clone(),
                    messages: messages.clone(),
                    settings: settings.clone(),
                    chats: chats.clone()
                }
            }

            if show_settings() {
                SettingsModal {
                    settings: settings.clone(),
                    show_settings: show_settings.clone(),
                    chats: chats.clone(),
                    messages: messages.clone(),
                    current_chat_id: current_chat_id.clone()
                }
            }
        }
    }
}
