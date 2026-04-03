//! Chat and message types for the application model

use chrono::{DateTime, Utc};
use crate::whatsapp::{self, Jid};

/// Message delivery/read status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageStatus {
    /// Message is being sent
    #[default]
    Pending,
    /// Message sent to server
    Sent,
    /// Message delivered to recipient
    Delivered,
    /// Message read by recipient
    Read,
    /// Message failed to send
    Failed,
}

impl From<whatsapp::MessageStatus> for MessageStatus {
    fn from(status: whatsapp::MessageStatus) -> Self {
        match status {
            whatsapp::MessageStatus::Pending => Self::Pending,
            whatsapp::MessageStatus::Sent => Self::Sent,
            whatsapp::MessageStatus::Delivered => Self::Delivered,
            whatsapp::MessageStatus::Read => Self::Read,
            whatsapp::MessageStatus::Failed => Self::Failed,
        }
    }
}

/// A chat conversation in the model
#[derive(Debug, Clone)]
pub struct Chat {
    /// Unique identifier (JID)
    pub jid: Jid,
    /// Display name
    pub name: String,
    /// Preview of last message
    pub last_message: String,
    /// Number of unread messages
    pub unread_count: u32,
    /// Whether this chat is pinned
    pub is_pinned: bool,
}

impl From<whatsapp::Chat> for Chat {
    fn from(chat: whatsapp::Chat) -> Self {
        Self {
            jid: chat.jid,
            name: chat.name,
            last_message: chat.last_message.unwrap_or_default(),
            unread_count: chat.unread_count,
            is_pinned: chat.is_pinned,
        }
    }
}

/// A message in a chat
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Unique message ID
    pub id: String,
    /// Whether this message was sent by the current user
    pub is_from_me: bool,
    /// Message content (text preview)
    pub content: String,
    /// When the message was sent
    pub timestamp: DateTime<Utc>,
    /// Delivery status
    pub status: MessageStatus,
}

impl ChatMessage {
    /// Create a new pending outgoing message
    pub fn new_outgoing(content: String) -> Self {
        Self::new_outgoing_with_id(
            format!("pending_{}", Utc::now().timestamp_millis()),
            content,
        )
    }

    /// Create a new pending outgoing message with a provided local ID
    pub fn new_outgoing_with_id(id: String, content: String) -> Self {
        Self {
            id,
            is_from_me: true,
            content,
            timestamp: Utc::now(),
            status: MessageStatus::Pending,
        }
    }
}

impl From<whatsapp::ChatMessage> for ChatMessage {
    fn from(msg: whatsapp::ChatMessage) -> Self {
        Self {
            id: msg.id,
            is_from_me: msg.is_from_me,
            content: msg.content.preview(),
            timestamp: msg.timestamp,
            status: msg.status.into(),
        }
    }
}
