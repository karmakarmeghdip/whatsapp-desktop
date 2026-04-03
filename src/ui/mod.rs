//! UI components and state management

pub mod sidebar;
pub mod chat;
pub mod views;

use std::collections::HashMap;

use iced::widget::row;
use iced::Element;
use crate::core::types::{Chat, ChatMessage, Message};
use crate::whatsapp::{self, Jid, MessageStatus, TypingState};

/// Main UI state
pub struct State {
    /// All chats
    chats: Vec<Chat>,
    /// Currently selected chat
    selected_chat: Option<Jid>,
    /// Messages per chat (keyed by JID string)
    messages: HashMap<String, Vec<ChatMessage>>,
    /// Current input value
    input_value: String,
    /// Typing indicators (chat JID -> sender JID -> state)
    typing_indicators: HashMap<String, HashMap<String, TypingState>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            chats: Vec::new(),
            selected_chat: None,
            messages: HashMap::new(),
            input_value: String::new(),
            typing_indicators: HashMap::new(),
        }
    }
}

impl State {
    /// Select a chat
    pub fn select_chat(&mut self, jid: Jid) {
        self.selected_chat = Some(jid);
    }

    /// Set the input value
    pub fn set_input(&mut self, value: String) {
        self.input_value = value;
    }

    /// Send a message (called when user presses send)
    pub fn send_message(&mut self) {
        if self.input_value.is_empty() {
            return;
        }

        if let Some(chat_jid) = &self.selected_chat {
            // Create a pending message for immediate UI feedback
            let msg = ChatMessage {
                id: format!("pending_{}", chrono::Utc::now().timestamp_millis()),
                is_me: true,
                content: self.input_value.clone(),
                timestamp: chrono::Utc::now(),
                status: MessageStatus::Pending,
            };

            // Add to local messages
            self.messages
                .entry(chat_jid.0.clone())
                .or_default()
                .push(msg);

            // Clear input
            self.input_value.clear();

            // TODO: Send via WhatsApp connection
        }
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

    /// Update message status
    pub fn update_message_status(&mut self, message_id: &str, status: MessageStatus) {
        for msgs in self.messages.values_mut() {
            for msg in msgs.iter_mut() {
                if msg.id == message_id {
                    msg.status = status;
                    return;
                }
            }
        }
    }

    /// Set all chats
    pub fn set_chats(&mut self, chats: Vec<whatsapp::Chat>) {
        self.chats = chats.into_iter().map(Chat::from).collect();
    }

    /// Update a single chat
    pub fn update_chat(&mut self, chat: whatsapp::Chat) {
        let chat: Chat = chat.into();
        if let Some(existing) = self.chats.iter_mut().find(|c| c.jid == chat.jid) {
            *existing = chat;
        } else {
            self.chats.push(chat);
        }
    }

    /// Set typing indicator
    pub fn set_typing(&mut self, chat_jid: Jid, sender_jid: Jid, state: TypingState) {
        self.typing_indicators
            .entry(chat_jid.0)
            .or_default()
            .insert(sender_jid.0, state);
    }

    /// Get typing status for a chat
    pub fn get_typing(&self, chat_jid: &Jid) -> Option<&HashMap<String, TypingState>> {
        self.typing_indicators.get(&chat_jid.0)
    }

    /// Render the main view
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = sidebar::view(&self.chats, self.selected_chat.as_ref());

        let chat_area = if let Some(chat_jid) = &self.selected_chat {
            let chat_name = self.chats
                .iter()
                .find(|c| &c.jid == chat_jid)
                .map(|c| c.name.as_str())
                .unwrap_or("Unknown");

            let messages = self.messages
                .get(&chat_jid.0)
                .map(|m| m.as_slice())
                .unwrap_or(&[]);

            let typing = self.get_typing(chat_jid)
                .and_then(|t| t.values().find(|s| **s != TypingState::Idle))
                .copied();

            chat::view(chat_name, messages, &self.input_value, typing)
        } else {
            chat::empty_view()
        };

        row![sidebar, chat_area].into()
    }
}
