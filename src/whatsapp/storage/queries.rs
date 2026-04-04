//! Storage Queries
//!
//! Database query operations for loading data from storage.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use super::super::{Chat, Jid};
use super::models::{StoredMessage, status_from_str};
use super::schema::ensure_schema;

/// Load a snapshot of chats and messages from the database
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

/// Load contact name mappings from the database
fn load_contact_map(conn: &Connection) -> HashMap<String, String> {
    let mut map = HashMap::new();
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

/// Load chats from the database
fn load_chats(conn: &Connection, contact_map: &HashMap<String, String>) -> Vec<Chat> {
    let mut stmt = match conn.prepare(
        "
        SELECT c.jid, c.name, c.last_message, c.last_activity_ms, c.is_group, c.unread_count, c.is_muted, c.is_pinned
        FROM app_chats c
        INNER JOIN (
            SELECT chat_jid, COUNT(*) as msg_count
            FROM app_messages
            GROUP BY chat_jid
            HAVING msg_count > 0
        ) m ON c.jid = m.chat_jid
        ORDER BY c.is_pinned DESC, COALESCE(c.last_activity_ms, c.updated_at_ms) DESC
        "
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
            jid: Jid(jid),
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

/// Load recent messages from the database
fn load_messages(conn: &Connection) -> Vec<StoredMessage> {
    let mut stmt = match conn.prepare(
        "
        SELECT chat_jid, message_id, sender_jid, is_from_me, timestamp_ms, status, raw_message
        FROM (
            SELECT
                chat_jid, message_id, sender_jid, is_from_me, timestamp_ms, status, raw_message,
                ROW_NUMBER() OVER (PARTITION BY chat_jid ORDER BY timestamp_ms DESC) AS rn
            FROM app_messages
            -- Use covering index for better performance
            -- INDEXED BY idx_app_messages_chat_time
            
            -- Optimize window function with partial index
            -- WHERE timestamp_ms > (strftime('%s', 'now') - 30*24*60*60) * 1000
        )
        WHERE rn <= 120
        ORDER BY timestamp_ms ASC
        
        -- Optimize for index-only scan if possible
        -- INDEXED BY idx_app_messages_timestamp
        "
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

/// Normalize a JID to extract the user part
fn normalize_user_from_jid(jid: &str) -> String {
    jid.split('@')
        .next()
        .unwrap_or(jid)
        .split(':')
        .next()
        .unwrap_or(jid)
        .to_string()
}
