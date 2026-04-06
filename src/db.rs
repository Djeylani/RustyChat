use rusqlite::{params, Connection, Row};

/* ================= DATABASE ================= */

#[derive(Clone, Debug)]
pub struct Settings {
    pub model: String,
    pub embed_model: String,
    pub mcp_server_command: String,
    pub system_prompt: String,
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
            system_prompt TEXT,
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

    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN embed_model TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE settings ADD COLUMN mcp_server_command TEXT NOT NULL DEFAULT ''",
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
            "INSERT INTO settings (id, model, embed_model, mcp_server_command, system_prompt, temperature, top_p, max_tokens, zoom, maximized, window_width, window_height)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "", // no default model — user must pick one
                "",
                "",
                "",
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
        "SELECT model, embed_model, mcp_server_command, system_prompt, temperature, top_p, max_tokens, zoom, maximized, window_width, window_height FROM settings WHERE id = 1",
        [],
        |row: &Row| {
            Ok(Settings {
                model: row.get::<_, String>(0)?,
                embed_model: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                mcp_server_command: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                system_prompt: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                temperature: row.get::<_, Option<f64>>(4)?.unwrap_or(0.7),
                top_p: row.get::<_, Option<f64>>(5)?.unwrap_or(0.95),
                max_tokens: clamp_to_i32(row.get::<_, Option<i64>>(6)?.unwrap_or(512)),
                zoom: clamp_to_i32(row.get::<_, Option<i64>>(7)?.unwrap_or(100)),
                // always treat maximized as true on start (we still read DB value for compatibility)
                maximized: true,
                window_width: clamp_to_i32(row.get::<_, Option<i64>>(9)?.unwrap_or(1024)),
                window_height: clamp_to_i32(row.get::<_, Option<i64>>(10)?.unwrap_or(768)),
            })
        },
    )
    .unwrap()
}

pub fn save_settings(conn: &Connection, s: &Settings) {
    // ensure fields are within i32 bounds
    let max_tokens: i64 = s.max_tokens.into();
    let zoom: i64 = s.zoom.into();
    let width: i64 = s.window_width.into();
    let height: i64 = s.window_height.into();

    conn.execute(
        "UPDATE settings SET model = ?1, embed_model = ?2, mcp_server_command = ?3, system_prompt = ?4, temperature = ?5, top_p = ?6, max_tokens = ?7, zoom = ?8, maximized = ?9, window_width = ?10, window_height = ?11 WHERE id = 1",
        params![
            s.model,
            s.embed_model,
            s.mcp_server_command,
            s.system_prompt,
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
