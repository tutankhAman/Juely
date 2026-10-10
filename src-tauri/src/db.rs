//! SQLite migrations — exact schema recovered from Pluely 1.1.0 `.rodata`.
//!
//! Upstream registers these on `sqlite:pluely.db` via `tauri-plugin-sql`.

use tauri_plugin_sql::{Migration, MigrationKind};

pub const SYSTEM_PROMPTS: &str = r#"
CREATE TABLE IF NOT EXISTS system_prompts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    created_at TEXT DEFAULT (datetime('now')) NOT NULL,
    updated_at TEXT DEFAULT (datetime('now')) NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_system_prompts_name ON system_prompts(name);
CREATE TRIGGER IF NOT EXISTS update_system_prompts_timestamp
AFTER UPDATE ON system_prompts
FOR EACH ROW
WHEN OLD.updated_at = NEW.updated_at
BEGIN
    UPDATE system_prompts SET updated_at = datetime('now') WHERE id = NEW.id;
END;
"#;

pub const CHAT_HISTORY: &str = r#"
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    attached_files TEXT,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_conversation_id ON messages(conversation_id);
CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
CREATE INDEX IF NOT EXISTS idx_messages_conversation_timestamp ON messages(conversation_id, timestamp ASC);
CREATE INDEX IF NOT EXISTS idx_messages_conversation_role ON messages(conversation_id, role, timestamp ASC);
CREATE TRIGGER IF NOT EXISTS update_conversation_timestamp_on_message_insert
AFTER INSERT ON messages FOR EACH ROW
BEGIN UPDATE conversations SET updated_at = NEW.timestamp WHERE id = NEW.conversation_id; END;
CREATE TRIGGER IF NOT EXISTS update_conversation_timestamp_on_message_update
AFTER UPDATE ON messages FOR EACH ROW
BEGIN UPDATE conversations SET updated_at = NEW.timestamp WHERE id = NEW.conversation_id; END;

CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    context TEXT NOT NULL DEFAULT 'auto',
    language TEXT NOT NULL DEFAULT 'en',
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at TEXT,
    duration_seconds INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS meeting_lines (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    who TEXT NOT NULL,
    speaker INTEGER,
    text TEXT NOT NULL,
    start_time REAL NOT NULL DEFAULT 0,
    end_time REAL NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (meeting_id) REFERENCES meetings (id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_meeting_lines_meeting ON meeting_lines (meeting_id);
CREATE INDEX IF NOT EXISTS idx_meetings_started ON meetings (started_at DESC);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    mime_type TEXT NOT NULL DEFAULT '',
    size_bytes INTEGER NOT NULL DEFAULT 0,
    pages INTEGER NOT NULL DEFAULT 0,
    ocr_text TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_files_created ON files (created_at DESC);

CREATE TABLE IF NOT EXISTS prompt_files (
    prompt_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    PRIMARY KEY (prompt_id, file_id),
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS saved_notes (
    kind TEXT NOT NULL,
    ref_id TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (kind, ref_id)
);

CREATE TABLE IF NOT EXISTS meeting_speakers (
    meeting_id TEXT NOT NULL,
    speaker_key TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (meeting_id, speaker_key)
);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS cached_summaries (
    kind TEXT NOT NULL,
    ref_id TEXT NOT NULL,
    mode TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (kind, ref_id, mode)
);
CREATE INDEX IF NOT EXISTS idx_cached_summaries_ref ON cached_summaries (kind, ref_id);

ALTER TABLE system_prompts ADD COLUMN listen_slot INTEGER;
CREATE INDEX IF NOT EXISTS idx_system_prompts_listen_slot
ON system_prompts(listen_slot) WHERE listen_slot IS NOT NULL;

INSERT INTO system_prompts (name, prompt, listen_slot) VALUES
('General','You are a helpful AI assistant. Be concise, accurate, and friendly in your responses.',0),
('Interview','You are a discreet real-time interview copilot. The user is in a live job interview. Given the conversation transcript and/or the user''s question, suggest a strong, natural first-person answer the user can say out loud. Ground the answer in concrete experience, keep it under 150 words, structure it clearly (situation → action → result when relevant), and never mention that you are an AI or that the user is being assisted.',1),
('Coding','You are a real-time technical interview copilot. The user is in a live coding or system-design interview. Analyze the problem from the transcript or question, then give: (1) the key insight, (2) a clean, idiomatic solution in the most likely expected language with the time/space complexity, and (3) one clarifying question worth asking. Be precise and compact — the user is reading while speaking.',2),
('Translate','You are a real-time conversation translator. Detect the language of the latest transcript lines, translate them into the user''s language (default English), and, when the other person asked something, suggest a short reply the user can say, written in both languages. Format: the original line as a quote, then **Translated:** and, when useful, **Suggested reply:**.',3),
('Meeting','You are a real-time meeting copilot. From the live transcript, keep the user on top of the discussion: summarize the key points so far, call out decisions, owners and action items, and suggest what the user should say or ask next when helpful. Be brief and skimmable — short bullets over prose.',4);
"#;

/// Ordered migrations registered against `sqlite:pluely.db`.
pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create_system_prompts_table",
            sql: SYSTEM_PROMPTS,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "create_chat_history_tables",
            sql: CHAT_HISTORY,
            kind: MigrationKind::Up,
        },
    ]
}