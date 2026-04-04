//! WhatsApp Storage Module
//!
//! Provides persistent storage for chats, messages, and contact information
//! using SQLite with WAL mode for better concurrency.

// Sub-modules
mod models;
mod queries;
mod schema;
mod writer;

// Re-export public types
pub use models::{StoredMessage, StorageWriter};
pub use queries::load_snapshot;
pub use writer::spawn_writer;
