//! WhatsApp Storage Module
//!
//! Provides persistent storage for chats, messages, and contact information
//! using SQLite with WAL mode for better concurrency.

use std::path::Path;
use std::path::PathBuf;

// Sub-modules
mod models;
mod queries;
mod schema;
mod writer;

// Re-export public types
pub use models::{StoredMessage, StorageWriter};
pub use queries::load_snapshot;
pub use writer::spawn_writer;

/// Initialize storage and return a writer handle
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// A `StorageWriter` handle that can be used to persist data asynchronously
///
/// # Example
/// ```rust,ignore
/// let writer = storage::spawn_writer(db_path);
/// writer.persist_chat(chat, None);
/// ```
pub fn initialize(db_path: PathBuf) -> StorageWriter {
    spawn_writer(db_path)
}

/// Load all stored data from the database
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// A tuple containing:
/// - Vector of stored chats
/// - Vector of stored messages
///
/// # Example
/// ```rust,ignore
/// let (chats, messages) = storage::load_all(&db_path);
/// ```
pub fn load_all(db_path: &Path) -> (Vec<super::Chat>, Vec<StoredMessage>) {
    load_snapshot(db_path)
}

/// Compact the database to reclaim space
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// `Ok(())` if successful, `Err` otherwise
pub fn compact(db_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use rusqlite::Connection;
    
    let conn = Connection::open(db_path)?;
    conn.execute_batch("VACUUM;")?;
    Ok(())
}

/// Get database statistics
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// A `HashMap` containing database statistics
pub fn get_stats(db_path: &Path) -> Result<std::collections::HashMap<String, i64>, Box<dyn std::error::Error>> {
    use rusqlite::Connection;
    
    let conn = Connection::open(db_path)?;
    let mut stats = std::collections::HashMap::new();
    
    // Get table counts
    let mut stmt = conn.prepare("SELECT count(*) FROM app_chats")?;
    let chat_count: i64 = stmt.query_row([], |row| row.get(0))?;
    stats.insert("chat_count".to_string(), chat_count);
    
    let mut stmt = conn.prepare("SELECT count(*) FROM app_messages")?;
    let message_count: i64 = stmt.query_row([], |row| row.get(0))?;
    stats.insert("message_count".to_string(), message_count);
    
    let mut stmt = conn.prepare("SELECT count(*) FROM app_contact_names")?;
    let contact_count: i64 = stmt.query_row([], |row| row.get(0))?;
    stats.insert("contact_count".to_string(), contact_count);
    
    // Get database file size
    if let Ok(metadata) = std::fs::metadata(db_path) {
        stats.insert("file_size_bytes".to_string(), metadata.len() as i64);
    }
    
    Ok(stats)
}
