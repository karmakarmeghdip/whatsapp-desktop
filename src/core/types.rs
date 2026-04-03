//! Core types for the WhatsApp desktop application
//!
//! These types are used throughout the application for state management
//! and message passing between components.

use crate::whatsapp::{self, WhatsAppEvent, Jid};

/// Application message enum for iced update cycle
#[derive(Debug, Clone)]
pub enum Message {
    // UI interactions
    SelectChat(Jid),
    InputChanged(String),
    SendMessage,

    // WhatsApp events (from subscription)
    WhatsApp(WhatsAppEvent),

    // Navigation
    ShowSettings,
    ShowPairing,
    BackToChats,
}

/// Simplified chat representation for UI
#[derive(Debug, Clone)]
pub struct Chat {
    pub jid: Jid,
    pub name: String,
    pub last_message: String,
    pub unread_count: u32,
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

/// Simplified message representation for UI
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub is_me: bool,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: whatsapp::MessageStatus,
}

impl From<whatsapp::ChatMessage> for ChatMessage {
    fn from(msg: whatsapp::ChatMessage) -> Self {
        Self {
            id: msg.id,
            is_me: msg.is_from_me,
            content: msg.content.preview(),
            timestamp: msg.timestamp,
            status: msg.status,
        }
    }
}

/// View state for the application
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewState {
    /// Loading/initializing
    Loading,
    /// Need to pair with QR code
    Pairing,
    /// Main chat view
    Chats,
    /// Settings view
    Settings,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState::Loading
    }
}
