use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
    Filesystem,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpKeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: McpTransport,
    pub target: String,
    pub auth_header_name: String,
    pub auth_token: String,
    pub custom_headers: Vec<McpKeyValue>,
    pub env_vars: Vec<McpKeyValue>,
}

/* ================= DATABASE ================= */

#[derive(Clone, Debug)]
pub struct Settings {
    pub model: String,
    pub embed_model: String,
    pub mcp_server_command: String,
    pub mcp_servers: Vec<McpServerConfig>,
    pub active_mcp_server_id: String,
    pub system_prompt: String,
    pub allow_code_execution: bool,
    pub execution_timeout_secs: i32,
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: i32,
    pub zoom: i32,
    pub maximized: bool,
    pub window_width: i32,
    pub window_height: i32,
}

pub fn init_db() -> Connection {
    let conn = Connection::open("chat.db").unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS chats (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            model TEXT NOT NULL,
            embed_model TEXT NOT NULL DEFAULT '',
            mcp_server_command TEXT NOT NULL DEFAULT '',
            mcp_servers_json TEXT NOT NULL DEFAULT '[]',
            active_mcp_server_id TEXT NOT NULL DEFAULT '',
            system_prompt TEXT,
            allow_code_execution INTEGER NOT NULL DEFAULT 0,
            execution_timeout_secs INTEGER NOT NULL DEFAULT 12,
            temperature REAL,
            top_p REAL,
            max_tokens INTEGER,
            zoom INTEGER,
            maximized INTEGER,
            window_width INTEGER,
            window_height INTEGER
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            level TEXT NOT NULL,
            source TEXT NOT NULL,
            message TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .unwrap();

    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN embed_model TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN mcp_server_command TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN mcp_servers_json TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN active_mcp_server_id TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN allow_code_execution INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN execution_timeout_secs INTEGER NOT NULL DEFAULT 12",
        [],
    );

    conn.execute(
        "CREATE TABLE IF NOT EXISTS document_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL,
            content TEXT NOT NULL,
            embedding BLOB NOT NULL
        )",
        [],
    )
    .unwrap();

    let exists: bool = conn
        .prepare("SELECT EXISTS(SELECT 1 FROM settings WHERE id = 1)")
        .unwrap()
        .query_row([], |r| r.get(0))
        .unwrap_or(false);

    if !exists {
        conn.execute(
            "INSERT INTO settings (id, model, embed_model, mcp_server_command, mcp_servers_json, active_mcp_server_id, system_prompt, allow_code_execution, execution_timeout_secs, temperature, top_p, max_tokens, zoom, maximized, window_width, window_height)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                "", // no default model — user must pick one
                "",
                "",
                "[]",
                "",
                "",
                0_i32,
                12_i32,
                0.7_f64,
                0.95_f64,
                512_i32,
                100_i32, // zoom %
                1_i32,   // maximized true by default
                1024_i32,
                768_i32
            ],
        )
        .unwrap();
    }

    conn
}

// clamp helper to ensure DB integer values respect Rust i32 bounds
pub fn clamp_to_i32(v: i64) -> i32 {
    if v > i32::MAX as i64 {
        i32::MAX
    } else if v < i32::MIN as i64 {
        i32::MIN
    } else {
        v as i32
    }
}

pub fn load_settings(conn: &Connection) -> Settings {
    conn.query_row(
        "SELECT model, embed_model, mcp_server_command, mcp_servers_json, active_mcp_server_id, system_prompt, allow_code_execution, execution_timeout_secs, temperature, top_p, max_tokens, zoom, maximized, window_width, window_height FROM settings WHERE id = 1",
        [],
        |row: &Row| {
            let legacy_mcp_command = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let parsed_servers = row.get::<_, Option<String>>(3)?.unwrap_or_else(|| "[]".to_string());
            let mut mcp_servers = serde_json::from_str::<Vec<McpServerConfig>>(&parsed_servers)
                .unwrap_or_default();
            if mcp_servers.is_empty() && !legacy_mcp_command.trim().is_empty() {
                mcp_servers.push(legacy_mcp_server(&legacy_mcp_command));
            }

            let active_mcp_server_id = row.get::<_, Option<String>>(4)?.unwrap_or_default();
            let active_mcp_server_id = if !active_mcp_server_id.is_empty()
                && mcp_servers.iter().any(|server| server.id == active_mcp_server_id)
            {
                active_mcp_server_id
            } else {
                mcp_servers
                    .first()
                    .map(|server| server.id.clone())
                    .unwrap_or_default()
            };

            Ok(Settings {
                model: row.get::<_, String>(0)?,
                embed_model: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                mcp_server_command: legacy_mcp_command,
                mcp_servers,
                active_mcp_server_id,
                system_prompt: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                allow_code_execution: row.get::<_, Option<i64>>(6)?.unwrap_or(0) != 0,
                execution_timeout_secs: clamp_to_i32(row.get::<_, Option<i64>>(7)?.unwrap_or(12)),
                temperature: row.get::<_, Option<f64>>(8)?.unwrap_or(0.7),
                top_p: row.get::<_, Option<f64>>(9)?.unwrap_or(0.95),
                max_tokens: clamp_to_i32(row.get::<_, Option<i64>>(10)?.unwrap_or(512)),
                zoom: clamp_to_i32(row.get::<_, Option<i64>>(11)?.unwrap_or(100)),
                // always treat maximized as true on start (we still read DB value for compatibility)
                maximized: true,
                window_width: clamp_to_i32(row.get::<_, Option<i64>>(13)?.unwrap_or(1024)),
                window_height: clamp_to_i32(row.get::<_, Option<i64>>(14)?.unwrap_or(768)),
            })
        },
    )
    .unwrap()
}

pub fn save_settings(conn: &Connection, s: &Settings) {
    // ensure fields are within i32 bounds
    let execution_timeout_secs = s.execution_timeout_secs.clamp(3, 120);
    let max_tokens: i64 = s.max_tokens.into();
    let zoom: i64 = s.zoom.into();
    let width: i64 = s.window_width.into();
    let height: i64 = s.window_height.into();
    let active_server = s.active_mcp_server();
    let legacy_command = active_server
        .as_ref()
        .map(|server| server.target.clone())
        .unwrap_or_default();
    let mcp_servers_json = serde_json::to_string(&s.mcp_servers).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "UPDATE settings SET model = ?1, embed_model = ?2, mcp_server_command = ?3, mcp_servers_json = ?4, active_mcp_server_id = ?5, system_prompt = ?6, allow_code_execution = ?7, execution_timeout_secs = ?8, temperature = ?9, top_p = ?10, max_tokens = ?11, zoom = ?12, maximized = ?13, window_width = ?14, window_height = ?15 WHERE id = 1",
        params![
            s.model,
            s.embed_model,
            legacy_command,
            mcp_servers_json,
            s.active_mcp_server_id,
            s.system_prompt,
            if s.allow_code_execution { 1 } else { 0 },
            execution_timeout_secs,
            s.temperature,
            s.top_p,
            clamp_to_i32(max_tokens),
            clamp_to_i32(zoom),
            if s.maximized { 1 } else { 0 },
            clamp_to_i32(width),
            clamp_to_i32(height)
        ],
    )
    .unwrap();
}

impl Settings {
    pub fn active_mcp_server(&self) -> Option<McpServerConfig> {
        self.mcp_servers
            .iter()
            .find(|server| server.id == self.active_mcp_server_id)
            .cloned()
            .or_else(|| self.mcp_servers.first().cloned())
    }
}

fn legacy_mcp_server(command: &str) -> McpServerConfig {
    let trimmed = command.trim();
    let transport = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        McpTransport::Http
    } else if std::path::Path::new(trimmed).is_dir() {
        McpTransport::Filesystem
    } else {
        McpTransport::Stdio
    };

    McpServerConfig {
        id: "legacy-default".to_string(),
        name: "Legacy MCP".to_string(),
        transport,
        target: trimmed.to_string(),
        auth_header_name: String::new(),
        auth_token: String::new(),
        custom_headers: Vec::new(),
        env_vars: Vec::new(),
    }
}

pub fn clear_document_chunks(conn: &Connection) -> usize {
    conn.execute("DELETE FROM document_chunks", [])
        .unwrap_or(0)
}

pub fn clear_document_chunks_for_prefix(conn: &Connection, path_prefix: &str) -> usize {
    let like_pattern = format!("{path_prefix}%");
    conn.execute(
        "DELETE FROM document_chunks WHERE file_path LIKE ?1",
        params![like_pattern],
    )
    .unwrap_or(0)
}

pub fn count_document_chunks(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
        .unwrap_or(0)
}

pub fn count_indexed_files(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(DISTINCT file_path) FROM document_chunks",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub fn count_chat_messages(conn: &Connection, chat_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
        params![chat_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub fn load_chat_messages(conn: &Connection, chat_id: &str, limit: i64) -> Vec<(String, String)> {
    let mut stmt = match conn.prepare(
        "SELECT role, content FROM messages
         WHERE chat_id = ?1 ORDER BY id DESC LIMIT ?2"
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map(params![chat_id, limit], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut collected: Vec<(String, String)> = rows.filter_map(Result::ok).collect();
    collected.reverse();
    collected
}

pub fn log_app_event(conn: &Connection, level: &str, source: &str, message: &str) {
    let _ = conn.execute(
        "INSERT INTO app_logs (level, source, message) VALUES (?1, ?2, ?3)",
        params![level, source, message],
    );
}

pub fn log_app_error(conn: &Connection, source: &str, message: &str) {
    log_app_event(conn, "error", source, message);
}

/* Helper to enforce history length in DB per chat - deletes oldest messages beyond history limit */
pub fn enforce_history_limit(conn: &Connection, chat_id: &str, limit: i64) {
    // count messages first
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE chat_id = ?1",
            params![chat_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if count <= limit {
        return;
    }

    // get cutoff id (the id at position limit from newest)
    if let Ok(cutoff_id) = conn.query_row(
        "SELECT id FROM messages WHERE chat_id = ?1 ORDER BY id DESC LIMIT 1 OFFSET ?2",
        params![chat_id, limit - 1],
        |r| r.get::<_, i64>(0),
    ) {
        let _ = conn.execute(
            "DELETE FROM messages WHERE chat_id = ?1 AND id <= ?2",
            params![chat_id, cutoff_id],
        );
    }
}
