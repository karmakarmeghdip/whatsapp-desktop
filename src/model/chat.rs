//! Chat and message types for the application model

use chrono::{DateTime, Utc};
use crate::model::Jid;
use crate::rpc::{Chat as RpcChat, ChatMessage as RpcChatMessage, MessageStatus as RpcMessageStatus};

/// Message delivery/read status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageStatus {
    #[default]
    Pending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl From<RpcMessageStatus> for MessageStatus {
    fn from(status: RpcMessageStatus) -> Self {
        match status {
            RpcMessageStatus::Pending => Self::Pending,
            RpcMessageStatus::Sent => Self::Sent,
            RpcMessageStatus::Delivered => Self::Delivered,
            RpcMessageStatus::Read => Self::Read,
            RpcMessageStatus::Failed => Self::Failed,
        }
    }
}

/// A chat conversation in the model
#[derive(Debug, Clone)]
pub struct Chat {
    pub jid: Jid,
    pub name: String,
    pub last_message: String,
    pub unread_count: u32,
    pub is_pinned: bool,
}

impl From<RpcChat> for Chat {
    fn from(chat: RpcChat) -> Self {
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
    pub id: String,
    pub is_from_me: bool,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
}

impl ChatMessage {
    pub fn new_outgoing(content: String) -> Self {
        Self::new_outgoing_with_id(
            format!("pending_{}", Utc::now().timestamp_millis()),
            content,
        )
    }

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

impl From<RpcChatMessage> for ChatMessage {
    fn from(msg: RpcChatMessage) -> Self {
        Self {
            id: msg.id,
            is_from_me: msg.is_from_me,
            content: msg.content.preview(),
            timestamp: msg.timestamp,
            status: msg.status.into(),
        }
    }
}
