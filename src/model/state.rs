//! Central application state
//!
//! This is the main model that holds all application data.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::model::{Chat, ChatMessage, MessageStatus};
use crate::model::connection::{ConnectionState, ViewState};
use crate::rpc::{self, RpcClientHandle, Jid};

#[derive(Debug, Clone)]
struct TypingIndicator {
    state: TypingState,
    updated_at: DateTime<Utc>,
}

/// Typing indicator state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingState {
    Idle,
    Typing,
    Recording,
}

/// Central application state - the single source of truth
#[derive(Debug)]
pub struct AppState {
    // Connection state
    pub connection: ConnectionState,
    pub rpc_client: Option<RpcClientHandle>,
    pub view: ViewState,
    pub qr_code: Option<String>,
    pub qr_code_data: Option<iced::widget::qr_code::Data>,
    pub error: Option<String>,
    pub sync_in_progress: bool,
    pub sync_progress: Option<(u32, u32)>,
    sync_last_update: Option<DateTime<Utc>>,

    // Chat data
    pub chats: Vec<Chat>,
    pub selected_chat: Option<Jid>,
    pub messages: HashMap<String, Vec<ChatMessage>>,
    chat_preview_timestamps: HashMap<String, DateTime<Utc>>,

    // UI state
    pub input_value: String,
    typing_indicators: HashMap<String, HashMap<String, TypingIndicator>>,
    ignore_next_scroll_event: bool,
    history_request_state: HashMap<String, (String, DateTime<Utc>)>,
    pub loading_older_messages: bool,
    older_loading_updated_at: Option<DateTime<Utc>>,
    /// Whether the message list is scrolled to the bottom (within threshold)
    pub is_scrolled_to_bottom: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection: ConnectionState::default(),
            rpc_client: None,
            view: ViewState::default(),
            qr_code: None,
            qr_code_data: None,
            error: None,
            sync_in_progress: false,
            sync_progress: None,
            sync_last_update: None,
            chats: Vec::new(),
            selected_chat: None,
            messages: HashMap::new(),
            chat_preview_timestamps: HashMap::new(),
            input_value: String::new(),
            typing_indicators: HashMap::new(),
            ignore_next_scroll_event: false,
            history_request_state: HashMap::new(),
            loading_older_messages: false,
            older_loading_updated_at: None,
            is_scrolled_to_bottom: true, // Start at bottom
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select_chat(&mut self, jid: Jid) {
        self.selected_chat = Some(jid);
        self.ignore_next_scroll_event = true;
        self.loading_older_messages = false;
        self.older_loading_updated_at = None;
        self.is_scrolled_to_bottom = true; // Reset to bottom when switching chats
    }

    pub fn consume_scroll_ignore_flag(&mut self) -> bool {
        if self.ignore_next_scroll_event {
            self.ignore_next_scroll_event = false;
            true
        } else {
            false
        }
    }

    /// Update scroll position tracking. Returns true if user is near bottom.
    pub fn update_scroll_position(&mut self, relative_offset_y: f32) -> bool {
        // Consider "at bottom" if within 5% of the end
        self.is_scrolled_to_bottom = relative_offset_y >= 0.95;
        self.is_scrolled_to_bottom
    }

    /// Check if we should auto-scroll (user is at bottom)
    pub fn should_auto_scroll(&self) -> bool {
        self.is_scrolled_to_bottom
    }

    pub fn selected_chat_history_cursor(&self) -> Option<(Jid, String, bool, i64)> {
        let chat = self.selected_chat.as_ref()?.clone();
        let list = self.messages.get(&chat.0)?;
        let oldest = list.iter().min_by_key(|m| m.timestamp)?;

        Some((
            chat,
            oldest.id.clone(),
            oldest.is_from_me,
            oldest.timestamp.timestamp_millis(),
        ))
    }

    pub fn start_older_history_request_if_allowed(
        &mut self,
        chat_jid: &Jid,
        oldest_msg_id: &str,
    ) -> bool {
        let now = Utc::now();

        if let Some((last_oldest_id, requested_at)) = self.history_request_state.get(&chat_jid.0)
        {
            let is_same_cursor = last_oldest_id == oldest_msg_id;
            let is_recent = (now - *requested_at) < chrono::Duration::seconds(3);
            if is_same_cursor && is_recent {
                return false;
            }
        }

        self.history_request_state.insert(
            chat_jid.0.clone(),
            (oldest_msg_id.to_string(), now),
        );
        self.loading_older_messages = true;
        self.older_loading_updated_at = Some(now);
        true
    }

    pub fn selected_chat(&self) -> Option<&Chat> {
        self.selected_chat.as_ref().and_then(|jid| {
            self.chats.iter().find(|c| &c.jid == jid)
        })
    }

    pub fn selected_messages(&self) -> &[ChatMessage] {
        self.selected_chat
            .as_ref()
            .and_then(|jid| self.messages.get(&jid.0))
            .map(|m| m.as_slice())
            .unwrap_or(&[])
    }

    pub fn set_input(&mut self, value: String) {
        self.input_value = value;
    }

    pub fn take_message_to_send(&mut self) -> Option<(Jid, String)> {
        if self.input_value.is_empty() {
            return None;
        }

        let jid = self.selected_chat.clone()?;
        let text = std::mem::take(&mut self.input_value);
        Some((jid, text))
    }

    pub fn add_pending_message(&mut self, chat_jid: &Jid, content: String) -> String {
        let local_id = format!("pending_{}", Utc::now().timestamp_millis());
        let msg = ChatMessage::new_outgoing_with_id(local_id.clone(), content.clone());
        self.messages.entry(chat_jid.0.clone()).or_default().push(msg);

        self.update_chat_preview(chat_jid, content, Utc::now());
        local_id
    }

    pub fn add_rpc_message(&mut self, msg: rpc::ChatMessage) {
        let chat_jid = msg.chat.0.clone();
        let chat_msg: ChatMessage = msg.into();

        let messages = self.messages.entry(chat_jid.clone()).or_default();
        if messages.iter().any(|m| m.id == chat_msg.id) {
            return;
        }
        if messages
            .last()
            .is_none_or(|last| last.timestamp <= chat_msg.timestamp)
        {
            messages.push(chat_msg.clone());
        } else {
            let idx = messages
                .binary_search_by_key(&chat_msg.timestamp, |m| m.timestamp)
                .unwrap_or_else(|i| i);
            messages.insert(idx, chat_msg.clone());
        }

        let jid = Jid(chat_jid.clone());
        self.update_chat_preview(&jid, chat_msg.content.clone(), chat_msg.timestamp);

        if self
            .selected_chat
            .as_ref()
            .is_some_and(|selected| selected.0 == chat_jid)
        {
            self.loading_older_messages = false;
            self.older_loading_updated_at = None;
        }
    }

    pub fn update_message_status(&mut self, message_id: &str, status: MessageStatus) {
        for messages in self.messages.values_mut() {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.status = status;
                return;
            }
        }
    }

    pub fn update_specific_message_status(&mut self, chat_jid: &Jid, message_id: &str, status: MessageStatus) {
        if let Some(messages) = self.messages.get_mut(&chat_jid.0)
            && let Some(msg) = messages.iter_mut().find(|m| m.id == message_id)
        {
            msg.status = status;
        }
    }

    pub fn resolve_pending_message_id(&mut self, chat_jid: &Jid, local_id: &str, server_id: &str) {
        if let Some(messages) = self.messages.get_mut(&chat_jid.0)
            && let Some(msg) = messages.iter_mut().find(|m| m.id == local_id)
        {
            msg.id = server_id.to_string();
            msg.status = MessageStatus::Sent;
        }
    }

    pub fn set_chats_from_rpc(&mut self, chats: Vec<rpc::Chat>) {
        self.chats = chats.into_iter().map(Chat::from).collect();
        if !self.chats.is_empty() && self.can_show_chats_view() {
            self.view = ViewState::Chats;
        }
    }

    pub fn update_chat_from_rpc(&mut self, chat: rpc::Chat) {
        let chat: Chat = chat.into();
        if let Some(existing) = self.chats.iter_mut().find(|c| c.jid == chat.jid) {
            *existing = chat;
        } else {
            self.chats.push(chat);
        }

        if self.can_show_chats_view() {
            self.view = ViewState::Chats;
        }
    }

    pub fn update_contact_name(&mut self, jid: &Jid, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }

        let normalized = jid.normalized_user();
        let mut updated = false;
        for chat in &mut self.chats {
            let same_jid = chat.jid == *jid;
            let same_user = chat.jid.normalized_user() == normalized;

            if (same_jid || same_user) && chat.name != trimmed {
                chat.name = trimmed.to_string();
                updated = true;
            }
        }

        let _ = updated;
    }

    pub fn set_typing(&mut self, chat_jid: Jid, sender_jid: Jid, state: TypingState) {
        self.typing_indicators
            .entry(chat_jid.0)
            .or_default()
            .insert(
                sender_jid.0,
                TypingIndicator {
                    state,
                    updated_at: Utc::now(),
                },
            );
    }

    pub fn selected_typing_state(&self) -> Option<TypingState> {
        self.selected_chat.as_ref().and_then(|jid| {
            self.typing_indicators
                .get(&jid.0)
                .and_then(|indicators| {
                    indicators.values()
                        .find(|entry| entry.state != TypingState::Idle)
                        .map(|entry| entry.state)
                })
        })
    }

    pub fn set_rpc_client(&mut self, client: RpcClientHandle) {
        self.rpc_client = Some(client);
    }

    pub fn clear_rpc_client(&mut self) {
        self.rpc_client = None;
    }

    pub fn set_connection_state(&mut self, state: ConnectionState) {
        match &state {
            ConnectionState::Connected => {
                self.view = ViewState::Chats;
                self.qr_code = None;
                self.qr_code_data = None;
                self.error = None;
            }
            ConnectionState::WaitingForQr { qr_code } => {
                self.view = ViewState::Pairing;
                self.qr_code = Some(qr_code.clone());
                self.qr_code_data = iced::widget::qr_code::Data::new(qr_code).ok();
            }
            ConnectionState::WaitingForPairCode { .. } => {
                self.view = ViewState::Pairing;
            }
            ConnectionState::LoggedOut => {
                self.view = ViewState::Pairing;
                self.qr_code = None;
                self.qr_code_data = None;
            }
            ConnectionState::Disconnected => {
                self.view = ViewState::Loading;
            }
            _ => {}
        }
        self.connection = state;
    }

    pub fn set_sync_progress(&mut self, current: u32, total: u32) {
        self.sync_in_progress = true;
        self.sync_progress = Some((current, total.max(current)));
        self.sync_last_update = Some(Utc::now());
    }

    pub fn finish_sync(&mut self) {
        self.sync_in_progress = false;
        self.sync_progress = None;
        self.sync_last_update = None;
    }

    pub fn cleanup_temporary_state(&mut self) {
        let now = Utc::now();

        self.typing_indicators.retain(|_, indicators| {
            indicators.retain(|_, entry| {
                entry.state != TypingState::Idle
                    && (now - entry.updated_at) < chrono::Duration::seconds(8)
            });
            !indicators.is_empty()
        });

        if self.sync_in_progress
            && self
                .sync_last_update
                .is_some_and(|last| (now - last) > chrono::Duration::seconds(8))
        {
            self.finish_sync();
        }

        if self.loading_older_messages
            && self
                .older_loading_updated_at
                .is_some_and(|last| (now - last) > chrono::Duration::seconds(6))
        {
            self.loading_older_messages = false;
            self.older_loading_updated_at = None;
        }
    }

    fn update_chat_preview(&mut self, chat_jid: &Jid, preview: String, timestamp: DateTime<Utc>) {
        if let Some(last_ts) = self.chat_preview_timestamps.get(&chat_jid.0)
            && timestamp < *last_ts
        {
            return;
        }

        self.chat_preview_timestamps
            .insert(chat_jid.0.clone(), timestamp);

        if let Some(chat) = self.chats.iter_mut().find(|c| c.jid == *chat_jid) {
            chat.last_message = preview;
            chat.last_activity = Some(timestamp);
        } else {
            self.chats.push(Chat {
                jid: chat_jid.clone(),
                name: chat_jid.display_label(),
                last_message: preview,
                last_activity: Some(timestamp),
                unread_count: 0,
                is_pinned: false,
            });
        }

        if self.can_show_chats_view() {
            self.view = ViewState::Chats;
        }
    }

    fn can_show_chats_view(&self) -> bool {
        !matches!(
            self.connection,
            ConnectionState::WaitingForQr { .. }
                | ConnectionState::WaitingForPairCode { .. }
                | ConnectionState::LoggedOut
        )
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }
}
