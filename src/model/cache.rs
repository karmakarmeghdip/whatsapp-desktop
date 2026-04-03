use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::whatsapp::Jid;

use super::chat::{Chat, ChatMessage, MessageStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedMessage {
    id: String,
    is_from_me: bool,
    content: String,
    timestamp_unix: i64,
    status: CachedMessageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CachedMessageStatus {
    Pending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl From<MessageStatus> for CachedMessageStatus {
    fn from(value: MessageStatus) -> Self {
        match value {
            MessageStatus::Pending => Self::Pending,
            MessageStatus::Sent => Self::Sent,
            MessageStatus::Delivered => Self::Delivered,
            MessageStatus::Read => Self::Read,
            MessageStatus::Failed => Self::Failed,
        }
    }
}

impl From<CachedMessageStatus> for MessageStatus {
    fn from(value: CachedMessageStatus) -> Self {
        match value {
            CachedMessageStatus::Pending => Self::Pending,
            CachedMessageStatus::Sent => Self::Sent,
            CachedMessageStatus::Delivered => Self::Delivered,
            CachedMessageStatus::Read => Self::Read,
            CachedMessageStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedChat {
    jid: String,
    name: String,
    last_message: String,
    unread_count: u32,
    is_pinned: bool,
}

impl From<&Chat> for CachedChat {
    fn from(chat: &Chat) -> Self {
        Self {
            jid: chat.jid.0.clone(),
            name: chat.name.clone(),
            last_message: chat.last_message.clone(),
            unread_count: chat.unread_count,
            is_pinned: chat.is_pinned,
        }
    }
}

impl From<CachedChat> for Chat {
    fn from(chat: CachedChat) -> Self {
        Self {
            jid: Jid(chat.jid),
            name: chat.name,
            last_message: chat.last_message,
            unread_count: chat.unread_count,
            is_pinned: chat.is_pinned,
        }
    }
}

impl From<&ChatMessage> for CachedMessage {
    fn from(message: &ChatMessage) -> Self {
        Self {
            id: message.id.clone(),
            is_from_me: message.is_from_me,
            content: message.content.clone(),
            timestamp_unix: message.timestamp.timestamp(),
            status: message.status.into(),
        }
    }
}

impl From<CachedMessage> for ChatMessage {
    fn from(message: CachedMessage) -> Self {
        Self {
            id: message.id,
            is_from_me: message.is_from_me,
            content: message.content,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(message.timestamp_unix, 0)
                .unwrap_or_else(chrono::Utc::now),
            status: message.status.into(),
        }
    }
}

fn cache_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("whatsapp-desktop").join("chats_cache.json")
}

fn json_cache_enabled() -> bool {
    std::env::var("WA_USE_JSON_CACHE").ok().as_deref() != Some("0")
}

fn messages_cache_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("whatsapp-desktop").join("messages_cache.json")
}

pub fn load_chats() -> Vec<Chat> {
    if !json_cache_enabled() {
        return Vec::new();
    }

    let path = cache_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };

    let cached: Vec<CachedChat> = match serde_json::from_str(&content) {
        Ok(chats) => chats,
        Err(error) => {
            log::warn!("Failed to parse chat cache at {}: {}", path.display(), error);
            return Vec::new();
        }
    };

    let chats: Vec<Chat> = cached.into_iter().map(Chat::from).collect();
    if !chats.is_empty() {
        log::info!("Loaded {} chats from cache", chats.len());
    }
    chats
}

pub fn save_chats(chats: &[Chat]) -> std::io::Result<()> {
    if !json_cache_enabled() {
        return Ok(());
    }

    let path = cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let cached: Vec<CachedChat> = chats.iter().map(CachedChat::from).collect();
    let data = serde_json::to_vec_pretty(&cached)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    std::fs::write(path, data)
}

pub fn load_messages() -> std::collections::HashMap<String, Vec<ChatMessage>> {
    if !json_cache_enabled() {
        return std::collections::HashMap::new();
    }

    let path = messages_cache_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return std::collections::HashMap::new(),
    };

    let cached: std::collections::HashMap<String, Vec<CachedMessage>> =
        match serde_json::from_str(&content) {
            Ok(messages) => messages,
            Err(error) => {
                log::warn!("Failed to parse messages cache at {}: {}", path.display(), error);
                return std::collections::HashMap::new();
            }
        };

    cached
        .into_iter()
        .map(|(jid, messages)| {
            let mut messages: Vec<ChatMessage> = messages.into_iter().map(ChatMessage::from).collect();
            messages.sort_by_key(|m| m.timestamp);
            (jid, messages)
        })
        .collect()
}

pub fn save_messages(
    messages: &std::collections::HashMap<String, Vec<ChatMessage>>,
) -> std::io::Result<()> {
    if !json_cache_enabled() {
        return Ok(());
    }

    let path = messages_cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let cached: std::collections::HashMap<String, Vec<CachedMessage>> = messages
        .iter()
        .map(|(jid, items)| {
            let mut items: Vec<CachedMessage> = items.iter().map(CachedMessage::from).collect();
            items.sort_by_key(|m| m.timestamp_unix);
            (jid.clone(), items)
        })
        .collect();

    let data = serde_json::to_vec_pretty(&cached)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    std::fs::write(path, data)
}
