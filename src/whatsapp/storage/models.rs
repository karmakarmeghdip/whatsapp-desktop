//! Storage Models
//!
//! Data models for the WhatsApp storage layer.

use std::sync::mpsc;

use super::super::{Chat, MessageStatus};

/// A stored message with all metadata
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

/// Internal storage commands
#[derive(Debug)]
pub(crate) enum StorageCommand {
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

/// Writer handle for async storage operations
#[derive(Clone, Debug)]
pub struct StorageWriter {
    tx: mpsc::Sender<StorageCommand>,
}

impl StorageWriter {
    pub(crate) fn new(tx: mpsc::Sender<StorageCommand>) -> Self {
        Self { tx }
    }

    /// Persist a chat to storage
    pub fn persist_chat(&self, chat: Chat, raw_conversation: Option<Vec<u8>>) {
        let _ = self.tx.send(StorageCommand::UpsertChat {
            chat,
            raw_conversation,
        });
    }

    /// Persist a message to storage
    pub fn persist_message(&self, message: StoredMessage) {
        let _ = self.tx.send(StorageCommand::UpsertMessage(message));
    }

    /// Persist a contact name mapping
    pub fn persist_contact_name(&self, jid: String, name: String) {
        let _ = self.tx.send(StorageCommand::UpsertContactName { jid, name });
    }
}

/// Convert MessageStatus to database string representation
pub fn status_to_str(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Pending => "pending",
        MessageStatus::Sent => "sent",
        MessageStatus::Delivered => "delivered",
        MessageStatus::Read => "read",
        MessageStatus::Failed => "failed",
    }
}

/// Parse MessageStatus from database string
pub fn status_from_str(value: &str) -> MessageStatus {
    match value {
        "pending" => MessageStatus::Pending,
        "sent" => MessageStatus::Sent,
        "delivered" => MessageStatus::Delivered,
        "read" => MessageStatus::Read,
        "failed" => MessageStatus::Failed,
        _ => MessageStatus::Delivered,
    }
}
