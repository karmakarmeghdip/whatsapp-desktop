//! Central application state
//!
//! This is the main model that holds all application data.

use std::collections::HashMap;
use crate::whatsapp::{self, Connection, Jid, TypingState};
use super::chat::{Chat, ChatMessage, MessageStatus};
use super::connection::{ConnectionState, ViewState};

/// Central application state - the single source of truth
#[derive(Debug, Default)]
pub struct AppState {
    // Connection state
    /// Current connection to WhatsApp
    pub connection: ConnectionState,
    /// Handle for sending commands to WhatsApp
    pub whatsapp: Option<Connection>,
    /// Which view to display
    pub view: ViewState,
    /// QR code data (if available for pairing)
    pub qr_code: Option<String>,
    /// Current error message (if any)
    pub error: Option<String>,

    // Chat data
    /// All chat conversations
    pub chats: Vec<Chat>,
    /// Currently selected chat JID
    pub selected_chat: Option<Jid>,
    /// Messages indexed by chat JID string
    pub messages: HashMap<String, Vec<ChatMessage>>,

    // UI state
    /// Current text in the message input
    pub input_value: String,
    /// Typing indicators: chat_jid -> (sender_jid -> state)
    pub typing_indicators: HashMap<String, HashMap<String, TypingState>>,
}

impl AppState {
    /// Create a new empty state
    pub fn new() -> Self {
        Self::default()
    }

    // --- Chat selection ---

    /// Select a chat by JID
    pub fn select_chat(&mut self, jid: Jid) {
        self.selected_chat = Some(jid);
    }

    /// Get the currently selected chat
    pub fn selected_chat(&self) -> Option<&Chat> {
        self.selected_chat.as_ref().and_then(|jid| {
            self.chats.iter().find(|c| &c.jid == jid)
        })
    }

    /// Get messages for the selected chat
    pub fn selected_messages(&self) -> &[ChatMessage] {
        self.selected_chat
            .as_ref()
            .and_then(|jid| self.messages.get(&jid.0))
            .map(|m| m.as_slice())
            .unwrap_or(&[])
    }

    // --- Message input ---

    /// Update the input field value
    pub fn set_input(&mut self, value: String) {
        self.input_value = value;
    }

    /// Clear the input field
    pub fn clear_input(&mut self) {
        self.input_value.clear();
    }

    /// Get the JID and text for sending, clearing the input
    /// Returns None if no chat selected or input is empty
    pub fn take_message_to_send(&mut self) -> Option<(Jid, String)> {
        if self.input_value.is_empty() {
            return None;
        }

        let jid = self.selected_chat.clone()?;
        let text = std::mem::take(&mut self.input_value);
        Some((jid, text))
    }

    // --- Message management ---

    /// Add a pending outgoing message for immediate UI feedback
    pub fn add_pending_message(&mut self, chat_jid: &Jid, content: String) {
        let msg = ChatMessage::new_outgoing(content);
        self.messages
            .entry(chat_jid.0.clone())
            .or_default()
            .push(msg);
    }

    /// Add a received message
    pub fn add_message(&mut self, msg: whatsapp::ChatMessage) {
        let chat_jid = msg.chat.0.clone();
        let chat_msg: ChatMessage = msg.into();
        self.messages
            .entry(chat_jid)
            .or_default()
            .push(chat_msg);
    }

    /// Update a message's delivery status
    pub fn update_message_status(&mut self, message_id: &str, status: MessageStatus) {
        for messages in self.messages.values_mut() {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.status = status;
                return;
            }
        }
    }

    // --- Chat management ---

    /// Replace all chats
    pub fn set_chats(&mut self, chats: Vec<whatsapp::Chat>) {
        self.chats = chats.into_iter().map(Chat::from).collect();
    }

    /// Update or add a single chat
    pub fn update_chat(&mut self, chat: whatsapp::Chat) {
        let chat: Chat = chat.into();
        if let Some(existing) = self.chats.iter_mut().find(|c| c.jid == chat.jid) {
            *existing = chat;
        } else {
            self.chats.push(chat);
        }
    }

    // --- Typing indicators ---

    /// Set typing state for a user in a chat
    pub fn set_typing(&mut self, chat_jid: Jid, sender_jid: Jid, state: TypingState) {
        self.typing_indicators
            .entry(chat_jid.0)
            .or_default()
            .insert(sender_jid.0, state);
    }

    /// Get the active typing state for the selected chat (if any)
    pub fn selected_typing_state(&self) -> Option<TypingState> {
        self.selected_chat.as_ref().and_then(|jid| {
            self.typing_indicators
                .get(&jid.0)
                .and_then(|indicators| {
                    indicators.values()
                        .find(|s| **s != TypingState::Idle)
                        .copied()
                })
        })
    }

    // --- Connection state ---

    /// Set the WhatsApp connection handle
    pub fn set_whatsapp_connection(&mut self, connection: Connection) {
        self.whatsapp = Some(connection);
    }

    /// Clear the WhatsApp connection (on disconnect/logout)
    pub fn clear_whatsapp_connection(&mut self) {
        self.whatsapp = None;
    }

    /// Update connection state and adjust view accordingly
    pub fn set_connection_state(&mut self, state: ConnectionState) {
        match &state {
            ConnectionState::Connected => {
                self.view = ViewState::Chats;
                self.qr_code = None;
                self.error = None;
            }
            ConnectionState::WaitingForQr { qr_code } => {
                self.view = ViewState::Pairing;
                self.qr_code = Some(qr_code.clone());
            }
            ConnectionState::WaitingForPairCode { .. } => {
                self.view = ViewState::Pairing;
            }
            ConnectionState::LoggedOut => {
                self.view = ViewState::Pairing;
                self.qr_code = None;
            }
            ConnectionState::Disconnected => {
                self.view = ViewState::Loading;
            }
            _ => {}
        }
        self.connection = state;
    }

    /// Set an error message
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Clear the error message
    pub fn clear_error(&mut self) {
        self.error = None;
    }
}
