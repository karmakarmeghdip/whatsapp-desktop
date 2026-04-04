//! Message Manager
//!
//! Handles message-related operations in the application state.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::model::{ChatMessage, MessageStatus};
use crate::rpc::{self, Jid};

/// Manages message storage and operations for a chat
#[derive(Debug)]
pub struct MessageManager {
    messages: HashMap<String, Vec<ChatMessage>>,
}

impl Default for MessageManager {
    fn default() -> Self {
        Self {
            messages: HashMap::new(),
        }
    }
}

impl MessageManager {
    /// Get messages for a specific chat
    pub fn get_messages(&self, chat_jid: &Jid) -> Option<&[ChatMessage]> {
        self.messages.get(&chat_jid.0).map(|m| m.as_slice())
    }

    /// Get mutable messages for a specific chat
    pub fn get_messages_mut(&mut self, chat_jid: &Jid) -> Option<&mut Vec<ChatMessage>> {
        self.messages.get_mut(&chat_jid.0)
    }

    /// Add a message from RPC
    pub fn add_rpc_message(&mut self, msg: rpc::ChatMessage) -> Option<(String, ChatMessage)> {
        let chat_jid = msg.chat.0.clone();
        let chat_msg: ChatMessage = msg.into();

        let messages = self.messages.entry(chat_jid.clone()).or_default();
        
        // Check for duplicates
        if messages.iter().any(|m| m.id == chat_msg.id) {
            return None;
        }

        // Insert in sorted order by timestamp
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

        Some((chat_jid, chat_msg))
    }

    /// Add a pending message (optimistic UI update)
    pub fn add_pending_message(&mut self, chat_jid: &Jid, content: String) -> (String, ChatMessage) {
        let local_id = format!("pending_{}", Utc::now().timestamp_millis());
        let msg = ChatMessage::new_outgoing_with_id(local_id.clone(), content);
        self.messages.entry(chat_jid.0.clone()).or_default().push(msg.clone());
        (local_id, msg)
    }

    /// Update message status globally
    pub fn update_message_status(&mut self, message_id: &str, status: MessageStatus) -> bool {
        for messages in self.messages.values_mut() {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.status = status;
                return true;
            }
        }
        false
    }

    /// Update message status for a specific chat
    pub fn update_specific_message_status(
        &mut self,
        chat_jid: &Jid,
        message_id: &str,
        status: MessageStatus,
    ) -> bool {
        if let Some(messages) = self.messages.get_mut(&chat_jid.0) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
                msg.status = status;
                return true;
            }
        }
        false
    }

    /// Resolve a pending message ID to a server ID
    pub fn resolve_pending_message_id(
        &mut self,
        chat_jid: &Jid,
        local_id: &str,
        server_id: &str,
    ) -> bool {
        if let Some(messages) = self.messages.get_mut(&chat_jid.0) {
            if let Some(msg) = messages.iter_mut().find(|m| m.id == local_id) {
                msg.id = server_id.to_string();
                msg.status = MessageStatus::Sent;
                return true;
            }
        }
        false
    }

    /// Get the oldest message cursor for a chat (for history fetching)
    pub fn get_oldest_cursor(&self, chat_jid: &Jid) -> Option<(String, bool, i64)> {
        let messages = self.messages.get(&chat_jid.0)?;
        let oldest = messages.iter().min_by_key(|m| m.timestamp)?;

        Some((
            oldest.id.clone(),
            oldest.is_from_me,
            oldest.timestamp.timestamp_millis(),
        ))
    }

    /// Get the newest message timestamp for a chat
    pub fn get_newest_timestamp(&self, chat_jid: &Jid) -> Option<DateTime<Utc>> {
        let messages = self.messages.get(&chat_jid.0)?;
        messages.iter().max_by_key(|m| m.timestamp).map(|m| m.timestamp)
    }

    /// Check if a message exists
    pub fn has_message(&self, chat_jid: &Jid, message_id: &str) -> bool {
        self.messages
            .get(&chat_jid.0)
            .map(|msgs| msgs.iter().any(|m| m.id == message_id))
            .unwrap_or(false)
    }

    /// Get message count for a chat
    pub fn message_count(&self, chat_jid: &Jid) -> usize {
        self.messages.get(&chat_jid.0).map(|m| m.len()).unwrap_or(0)
    }

    /// Get total message count across all chats
    pub fn total_message_count(&self) -> usize {
        self.messages.values().map(|v| v.len()).sum()
    }

    /// Clear all messages (for logout/reset)
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Remove messages for a specific chat
    pub fn clear_chat(&mut self, chat_jid: &Jid) {
        self.messages.remove(&chat_jid.0);
    }

    /// Get all message IDs for a chat
    pub fn get_message_ids(&self, chat_jid: &Jid) -> Vec<String> {
        self.messages
            .get(&chat_jid.0)
            .map(|msgs| msgs.iter().map(|m| m.id.clone()).collect())
            .unwrap_or_default()
    }

    /// Search messages in a chat (simple substring search)
    pub fn search_messages(&self, chat_jid: &Jid, query: &str) -> Vec<&ChatMessage> {
        let query_lower = query.to_lowercase();
        self.messages
            .get(&chat_jid.0)
            .map(|msgs| {
                msgs.iter()
                    .filter(|m| m.content.to_string().to_lowercase().contains(&query_lower))
                    .collect()
            })
            .unwrap_or_default()
    }
}
