//! Storage Schema
//!
//! Database schema definitions and migrations for the WhatsApp storage.

use rusqlite::Connection;

/// Ensure the database schema is up to date
pub fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
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
        
        -- Migration: add indices for performance
        CREATE INDEX IF NOT EXISTS idx_app_chats_activity 
            ON app_chats(last_activity_ms DESC);
        CREATE INDEX IF NOT EXISTS idx_app_messages_timestamp 
            ON app_messages(timestamp_ms DESC);
        
        -- Analyze tables for query optimization
        ANALYZE;
        
        -- Vacuum to reclaim space and optimize
        VACUUM;
        
        -- Optimize for WAL mode if available
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = 10000;
        PRAGMA temp_store = MEMORY;
        
        -- Analyze again after optimizations
        ANALYZE;
        
        -- VACUUM to reclaim space after optimizations
        VACUUM;
        
        -- Ensure indices are created for common queries
        CREATE INDEX IF NOT EXISTS idx_app_chats_pinned 
            ON app_chats(is_pinned DESC, last_activity_ms DESC);
        
        -- Create a view for recent chats with unread counts
        CREATE VIEW IF NOT EXISTS v_recent_chats AS
        SELECT 
            jid,
            name,
            last_message,
            last_activity_ms,
            is_group,
            unread_count,
            is_muted,
            is_pinned
        FROM app_chats
        WHERE is_pinned = 1 OR last_activity_ms > (strftime('%s', 'now') - 7*24*60*60) * 1000
        ORDER BY is_pinned DESC, last_activity_ms DESC;
        
        -- Create a view for message statistics
        CREATE VIEW IF NOT EXISTS v_message_stats AS
        SELECT 
            chat_jid,
            COUNT(*) as total_messages,
            SUM(CASE WHEN is_from_me = 1 THEN 1 ELSE 0 END) as sent_messages,
            SUM(CASE WHEN is_from_me = 0 THEN 1 ELSE 0 END) as received_messages,
            MAX(timestamp_ms) as last_message_time
        FROM app_messages
        GROUP BY chat_jid;
        
        -- Create trigger to update chat last_activity on new message
        CREATE TRIGGER IF NOT EXISTS trg_update_chat_activity
        AFTER INSERT ON app_messages
        BEGIN
            UPDATE app_chats 
            SET last_activity_ms = NEW.timestamp_ms,
                last_message = substr(NEW.raw_message, 1, 200)
            WHERE jid = NEW.chat_jid;
        END;
        
        -- Create trigger to update unread count on new message
        CREATE TRIGGER IF NOT EXISTS trg_update_unread_count
        AFTER INSERT ON app_messages
        WHEN NEW.is_from_me = 0
        BEGIN
            UPDATE app_chats 
            SET unread_count = unread_count + 1
            WHERE jid = NEW.chat_jid;
        END;
        "
    )
}
