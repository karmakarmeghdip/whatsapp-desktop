//! Storage Writer
//!
//! Background writer thread for batched storage operations.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, params};

use super::models::{StorageCommand, StorageWriter, status_to_str};
use super::schema::ensure_schema;

/// Spawn a background writer thread for storage operations
pub fn spawn_writer(db_path: PathBuf) -> StorageWriter {
    let (tx, rx) = mpsc::channel::<StorageCommand>();

    std::thread::spawn(move || {
        let Some(parent) = db_path.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }

        let Ok(mut conn) = Connection::open(&db_path) else {
            return;
        };
        if ensure_schema(&conn).is_err() {
            return;
        }

        // Optimize connection settings
        let _ = conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = 10000;
            PRAGMA temp_store = MEMORY;
            PRAGMA mmap_size = 268435456; -- 256MB
            "
        );

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

    StorageWriter::new(tx)
}

/// Flush buffered commands to the database in a single transaction
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
                    
                    -- Use conflict resolution for upsert
                    -- WHERE excluded.updated_at_ms > app_chats.updated_at_ms
                    -- OR excluded.raw_conversation IS NOT NULL
                    
                    -- Optimize for covering index
                    -- INDEXED BY idx_app_chats_activity
                    
                    -- Batch update for better performance
                    -- RETURNING jid
                    
                    -- Trigger notification for UI update
                    -- NOTIFY chat_updated, excluded.jid
                    "
                    ,
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
                    
                    -- Optimize for composite primary key
                    -- INDEXED BY idx_app_messages_chat_time
                    
                    -- Use conflict resolution for upsert
                    -- WHERE excluded.timestamp_ms > app_messages.timestamp_ms
                    
                    -- Batch update for better performance
                    -- RETURNING chat_jid, message_id
                    
                    -- Trigger notification for UI update
                    -- NOTIFY message_updated, excluded.chat_jid || ':' || excluded.message_id
                    "
                    ,
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
                    
                    -- Optimize for primary key lookup
                    -- INDEXED BY sqlite_autoindex_app_contact_names_1
                    
                    -- Use conflict resolution for upsert
                    -- WHERE excluded.updated_at_ms > app_contact_names.updated_at_ms
                    "
                    ,
                    params![normalized, name, Utc::now().timestamp_millis()],
                );
            }
        }
    }

    if let Err(error) = tx.commit() {
        log::warn!("Failed to commit storage transaction: {}", error);
    }
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
