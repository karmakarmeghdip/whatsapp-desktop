//! WhatsApp types adapted for the desktop application

use chrono::{DateTime, Utc};

/// Represents a WhatsApp JID (Jabber ID)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Jid(pub String);

impl Jid {
    pub fn new(jid: impl Into<String>) -> Self {
        Self(jid.into())
    }

    /// Extract the user part of the JID (before @)
    pub fn user(&self) -> &str {
        self.0.split('@').next().unwrap_or(&self.0)
    }

    /// Check if this is a group JID
    pub fn is_group(&self) -> bool {
        self.0.contains("@g.us")
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
#[derive(Debug, Clone)]
pub struct Chat {
    /// Unique identifier (JID)
    pub jid: Jid,
    /// Display name (contact name or group name)
    pub name: String,
    /// Last message preview
    pub last_message: Option<String>,
    /// Last activity timestamp
    pub last_activity: Option<DateTime<Utc>>,
    /// Whether this is a group chat
    pub is_group: bool,
    /// Unread message count
    pub unread_count: u32,
    /// Whether the chat is muted
    pub is_muted: bool,
    /// Whether the chat is pinned
    pub is_pinned: bool,
}

impl Chat {
    pub fn new(jid: impl Into<Jid>, name: impl Into<String>) -> Self {
        let jid = jid.into();
        let is_group = jid.is_group();
        Self {
            jid,
            name: name.into(),
            last_message: None,
            last_activity: None,
            is_group,
            unread_count: 0,
            is_muted: false,
            is_pinned: false,
        }
    }
}

/// A message in a chat
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message ID
    pub id: String,
    /// Sender JID
    pub sender: Jid,
    /// Chat JID (where the message was sent)
    pub chat: Jid,
    /// Message content
    pub content: MessageContent,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Whether this message is from the current user
    pub is_from_me: bool,
    /// Message status
    pub status: MessageStatus,
    /// Quoted message (if this is a reply)
    pub quoted_message: Option<Box<ChatMessage>>,
}

/// Content types for messages
#[derive(Debug, Clone)]
pub enum MessageContent {
    /// Plain text message
    Text(String),
    /// Image message with optional caption
    Image {
        caption: Option<String>,
        /// Media URL (for download)
        url: Option<String>,
        /// Thumbnail data
        thumbnail: Option<Vec<u8>>,
    },
    /// Video message with optional caption
    Video {
        caption: Option<String>,
        url: Option<String>,
        thumbnail: Option<Vec<u8>>,
    },
    /// Audio/voice message
    Audio {
        url: Option<String>,
        duration_secs: u32,
        is_voice_note: bool,
    },
    /// Document/file message
    Document {
        filename: String,
        url: Option<String>,
        mime_type: Option<String>,
    },
    /// Sticker
    Sticker {
        url: Option<String>,
    },
    /// Location
    Location {
        latitude: f64,
        longitude: f64,
        name: Option<String>,
    },
    /// Contact card
    Contact {
        display_name: String,
        vcard: String,
    },
    /// System message (e.g., "X added Y to the group")
    System(String),
    /// Unknown/unsupported message type
    Unknown,
}

impl MessageContent {
    /// Get a preview string for the message content
    pub fn preview(&self) -> String {
        match self {
            MessageContent::Text(text) => {
                if text.len() > 50 {
                    format!("{}...", &text[..47])
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus {
    /// Message is being sent
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

/// Connection state of the WhatsApp client
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected, need to pair
    Disconnected,
    /// Connecting to WhatsApp servers
    Connecting,
    /// Waiting for QR code scan
    WaitingForQr { qr_code: String },
    /// Waiting for pair code entry on phone
    WaitingForPairCode { code: String },
    /// Successfully connected
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting,
    /// Logged out (need to re-pair)
    LoggedOut,
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    pub fn needs_pairing(&self) -> bool {
        matches!(
            self,
            ConnectionState::Disconnected
                | ConnectionState::WaitingForQr { .. }
                | ConnectionState::WaitingForPairCode { .. }
                | ConnectionState::LoggedOut
        )
    }
}

/// Typing indicator state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingState {
    /// Not typing
    Idle,
    /// Typing text
    Typing,
    /// Recording audio
    Recording,
}

/// Presence (online/offline) information
#[derive(Debug, Clone)]
pub struct Presence {
    pub jid: Jid,
    pub is_online: bool,
    pub last_seen: Option<DateTime<Utc>>,
}
