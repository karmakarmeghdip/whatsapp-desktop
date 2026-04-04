//! Serializable types for RPC communication
//!
//! These types mirror the whatsapp service types but are fully serializable
//! to support cross-process communication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a WhatsApp JID (Jabber ID)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Jid(pub String);

impl Jid {
    pub fn user(&self) -> &str {
        self.0.split('@').next().unwrap_or(&self.0)
    }

    pub fn normalized_user(&self) -> String {
        self.user().split(':').next().unwrap_or(self.user()).to_string()
    }

    pub fn display_label(&self) -> String {
        self.normalized_user()
    }
}

impl std::fmt::Display for Jid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Jid {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Jid {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// A chat/conversation in WhatsApp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub jid: Jid,
    pub name: String,
    pub last_message: Option<String>,
    pub last_activity: Option<DateTime<Utc>>,
    pub is_group: bool,
    pub unread_count: u32,
    pub is_muted: bool,
    pub is_pinned: bool,
}



/// A message in a chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender: Jid,
    pub chat: Jid,
    pub content: MessageContent,
    pub timestamp: DateTime<Utc>,
    pub is_from_me: bool,
    pub status: MessageStatus,
    pub quoted_message: Option<Box<ChatMessage>>,
}

/// Content types for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    Text(String),
    Image {
        caption: Option<String>,
        url: Option<String>,
        thumbnail: Option<Vec<u8>>,
    },
    Video {
        caption: Option<String>,
        url: Option<String>,
        thumbnail: Option<Vec<u8>>,
    },
    Audio {
        url: Option<String>,
        duration_secs: u32,
        is_voice_note: bool,
    },
    Document {
        filename: String,
        url: Option<String>,
        mime_type: Option<String>,
    },
    Sticker {
        url: Option<String>,
    },
    Location {
        latitude: f64,
        longitude: f64,
        name: Option<String>,
    },
    Contact {
        display_name: String,
        vcard: String,
    },
    System(String),
    Unknown,
}

impl MessageContent {
    pub fn preview(&self) -> String {
        match self {
            MessageContent::Text(text) => {
                let char_count = text.chars().count();
                if char_count > 50 {
                    let truncated: String = text.chars().take(47).collect();
                    format!("{}...", truncated)
                } else {
                    text.clone()
                }
            }
            MessageContent::Image { caption, .. } => {
                caption.clone().unwrap_or_else(|| "📷 Photo".to_string())
            }
            MessageContent::Video { caption, .. } => {
                caption.clone().unwrap_or_else(|| "🎥 Video".to_string())
            }
            MessageContent::Audio { is_voice_note, .. } => {
                if *is_voice_note {
                    "🎤 Voice message".to_string()
                } else {
                    "🎵 Audio".to_string()
                }
            }
            MessageContent::Document { filename, .. } => {
                format!("📄 {}", filename)
            }
            MessageContent::Sticker { .. } => "🎭 Sticker".to_string(),
            MessageContent::Location { name, .. } => {
                name.clone().unwrap_or_else(|| "📍 Location".to_string())
            }
            MessageContent::Contact { display_name, .. } => {
                format!("👤 {}", display_name)
            }
            MessageContent::System(text) => text.clone(),
            MessageContent::Unknown => "Unsupported message".to_string(),
        }
    }
}

/// Message delivery/read status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    Pending,
    Sent,
    Delivered,
    Read,
    Failed,
}

/// Connection state of the WhatsApp client
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    WaitingForQr { qr_code: String },
    WaitingForPairCode { code: String },
    Connected,
    Reconnecting,
    LoggedOut,
}



/// Typing indicator state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypingState {
    Idle,
    Typing,
    Recording,
}

/// Presence (online/offline) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presence {
    pub jid: Jid,
    pub is_online: bool,
    pub last_seen: Option<DateTime<Utc>>,
}
