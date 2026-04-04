use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

use super::{Chat, MessageStatus};

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub message_id: String,
    pub sender_jid: String,
    pub chat_jid: String,
    pub is_from_me: bool,
    pub timestamp_ms: i64,
    pub status: MessageStatus,
    pub raw_message: Vec<u8>,
}

#[derive(Debug)]
enum StorageCommand {
    UpsertChat {
        chat: Chat,
        raw_conversation: Option<Vec<u8>>,
    },
    UpsertMessage(StoredMessage),
    UpsertContactName {
        jid: String,
        name: String,
    },
}

#[derive(Clone, Debug)]
pub struct StorageWriter {
    tx: mpsc::Sender<StorageCommand>,
}

impl StorageWriter {
    pub fn persist_chat(&self, chat: Chat, raw_conversation: Option<Vec<u8>>) {
        let _ = self.tx.send(StorageCommand::UpsertChat {
            chat,
            raw_conversation,
        });
    }

    pub fn persist_message(&self, message: StoredMessage) {
        let _ = self.tx.send(StorageCommand::UpsertMessage(message));
    }

    pub fn persist_contact_name(&self, jid: String, name: String) {
        let _ = self.tx.send(StorageCommand::UpsertContactName { jid, name });
    }
}

pub fn spawn_writer(db_path: PathBuf) -> StorageWriter {
    let (tx, rx) = mpsc::channel::<StorageCommand>();

    std::thread::spawn(move || {
        let Some(parent) = db_path.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }

        let Ok(mut conn) = Connection::open(db_path) else {
            return;
        };
        if ensure_schema(&conn).is_err() {
            return;
        }

        let mut buffer = Vec::with_capacity(256);
        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(command) => {
                    buffer.push(command);
                    if buffer.len() >= 200 {
                        flush(&mut conn, &mut buffer);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !buffer.is_empty() {
                        flush(&mut conn, &mut buffer);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !buffer.is_empty() {
                        flush(&mut conn, &mut buffer);
                    }
                    break;
                }
            }
        }
    });

    StorageWriter { tx }
}

pub fn load_snapshot(db_path: &Path) -> (Vec<Chat>, Vec<StoredMessage>) {
    let Ok(conn) = Connection::open(db_path) else {
        return (Vec::new(), Vec::new());
    };
    if ensure_schema(&conn).is_err() {
        return (Vec::new(), Vec::new());
    }

    let contact_map = load_contact_map(&conn);
    let chats = load_chats(&conn, &contact_map);
    let messages = load_messages(&conn);

    (chats, messages)
}

fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS app_chats (
            jid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            last_message TEXT,
            last_activity_ms INTEGER,
            is_group INTEGER NOT NULL,
            unread_count INTEGER NOT NULL,
            is_muted INTEGER NOT NULL,
            is_pinned INTEGER NOT NULL,
            raw_conversation BLOB,
            updated_at_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS app_messages (
            chat_jid TEXT NOT NULL,
            message_id TEXT NOT NULL,
            sender_jid TEXT NOT NULL,
            is_from_me INTEGER NOT NULL,
            timestamp_ms INTEGER NOT NULL,
            status TEXT NOT NULL,
            raw_message BLOB NOT NULL,
            PRIMARY KEY(chat_jid, message_id)
        );

        CREATE INDEX IF NOT EXISTS idx_app_messages_chat_time
            ON app_messages(chat_jid, timestamp_ms DESC);

        CREATE TABLE IF NOT EXISTS app_contact_names (
            user_key TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        ",
    )
}

fn normalize_user_from_jid(jid: &str) -> String {
    jid.split('@')
        .next()
        .unwrap_or(jid)
        .split(':')
        .next()
        .unwrap_or(jid)
        .to_string()
}

fn status_to_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Pending => "pending",
        MessageStatus::Sent => "sent",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Read => "read",
        MessageStatus::Failed => "failed",
    }
}

fn status_from_str(value: &str) -> MessageStatus {
    match value {
        "pending" => MessageStatus::Pending,
        "sent" => MessageStatus::Sent,
        "delivered" => MessageStatus::Delivered,
        "read" => MessageStatus::Read,
        "failed" => MessageStatus::Failed,
        _ => MessageStatus::Delivered,
    }
}

fn flush(conn: &mut Connection, buffer: &mut Vec<StorageCommand>) {
    if buffer.is_empty() {
        return;
    }

    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(error) => {
            log::warn!("Failed to open storage transaction: {}", error);
            buffer.clear();
            return;
        }
    };

    for command in buffer.drain(..) {
        match command {
            StorageCommand::UpsertChat {
                chat,
                raw_conversation,
            } => {
                let _ = tx.execute(
                    "
                    INSERT INTO app_chats (
                        jid, name, last_message, last_activity_ms,
                        is_group, unread_count, is_muted, is_pinned,
                        raw_conversation, updated_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    ON CONFLICT(jid) DO UPDATE SET
                        name=excluded.name,
                        last_message=excluded.last_message,
                        last_activity_ms=excluded.last_activity_ms,
                        is_group=excluded.is_group,
                        unread_count=excluded.unread_count,
                        is_muted=excluded.is_muted,
                        is_pinned=excluded.is_pinned,
                        raw_conversation=COALESCE(excluded.raw_conversation, app_chats.raw_conversation),
                        updated_at_ms=excluded.updated_at_ms
                    ",
                    params![
                        chat.jid.0,
                        chat.name,
                        chat.last_message,
                        chat.last_activity.map(|d| d.timestamp_millis()),
                        i64::from(chat.is_group as i32),
                        chat.unread_count,
                        i64::from(chat.is_muted as i32),
                        i64::from(chat.is_pinned as i32),
                        raw_conversation,
                        Utc::now().timestamp_millis()
                    ],
                );
            }
            StorageCommand::UpsertMessage(message) => {
                let _ = tx.execute(
                    "
                    INSERT INTO app_messages (
                        chat_jid, message_id, sender_jid, is_from_me,
                        timestamp_ms, status, raw_message
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ON CONFLICT(chat_jid, message_id) DO UPDATE SET
                        sender_jid=excluded.sender_jid,
                        is_from_me=excluded.is_from_me,
                        timestamp_ms=excluded.timestamp_ms,
                        status=excluded.status,
                        raw_message=excluded.raw_message
                    ",
                    params![
                        message.chat_jid,
                        message.message_id,
                        message.sender_jid,
                        i64::from(message.is_from_me as i32),
                        message.timestamp_ms,
                        status_to_str(message.status),
                        message.raw_message,
                    ],
                );
            }
            StorageCommand::UpsertContactName { jid, name } => {
                let normalized = normalize_user_from_jid(&jid);
                let _ = tx.execute(
                    "
                    INSERT INTO app_contact_names (user_key, name, updated_at_ms)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT(user_key) DO UPDATE SET
                        name=excluded.name,
                        updated_at_ms=excluded.updated_at_ms
                    ",
                    params![normalized, name, Utc::now().timestamp_millis()],
                );
            }
        }
    }

    if let Err(error) = tx.commit() {
        log::warn!("Failed to commit storage transaction: {}", error);
    }
}

fn load_contact_map(conn: &Connection) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut stmt = match conn.prepare("SELECT user_key, name FROM app_contact_names") {
        Ok(stmt) => stmt,
        Err(_) => return map,
    };

    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return map,
    };

    for (user, name) in rows.filter_map(Result::ok) {
        map.insert(user, name);
    }

    map
}

fn load_chats(conn: &Connection, contact_map: &std::collections::HashMap<String, String>) -> Vec<Chat> {
    let mut stmt = match conn.prepare(
        "
        SELECT jid, name, last_message, last_activity_ms, is_group, unread_count, is_muted, is_pinned
        FROM app_chats
        ORDER BY is_pinned DESC, COALESCE(last_activity_ms, updated_at_ms) DESC
        ",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| {
        let jid: String = row.get(0)?;
        let mut name: String = row.get(1)?;
        let normalized = normalize_user_from_jid(&jid);
        if !jid.contains("@g.us")
            && let Some(contact_name) = contact_map.get(&normalized)
        {
            name = contact_name.clone();
        }

        Ok(Chat {
            jid: super::Jid(jid),
            name,
            last_message: row.get::<_, Option<String>>(2)?,
            last_activity: row
                .get::<_, Option<i64>>(3)?
                .and_then(DateTime::<Utc>::from_timestamp_millis),
            is_group: row.get::<_, i64>(4)? != 0,
            unread_count: row.get::<_, u32>(5)?,
            is_muted: row.get::<_, i64>(6)? != 0,
            is_pinned: row.get::<_, i64>(7)? != 0,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(Result::ok).collect()
}

fn load_messages(conn: &Connection) -> Vec<StoredMessage> {
    let mut stmt = match conn.prepare(
        "
        SELECT chat_jid, message_id, sender_jid, is_from_me, timestamp_ms, status, raw_message
        FROM (
            SELECT
                chat_jid, message_id, sender_jid, is_from_me, timestamp_ms, status, raw_message,
                ROW_NUMBER() OVER (PARTITION BY chat_jid ORDER BY timestamp_ms DESC) AS rn
            FROM app_messages
        )
        WHERE rn <= 120
        ORDER BY timestamp_ms ASC
        ",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| {
        Ok(StoredMessage {
            chat_jid: row.get(0)?,
            message_id: row.get(1)?,
            sender_jid: row.get(2)?,
            is_from_me: row.get::<_, i64>(3)? != 0,
            timestamp_ms: row.get(4)?,
            status: status_from_str(&row.get::<_, String>(5)?),
            raw_message: row.get(6)?,
        })
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(Result::ok).collect()
}
