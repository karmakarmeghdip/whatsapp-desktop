//! Chat Manager
//!
//! Handles chat-related operations in the application state.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::model::{Chat, MessageContent};
use crate::model::connection::ViewState;
use crate::rpc::{self, Jid};

/// Manages chat list and selection state
#[derive(Debug, Default)]
pub struct ChatManager {
    chats: Vec<Chat>,
    selected_chat: Option<Jid>,
    chat_preview_timestamps: HashMap<String, DateTime<Utc>>,
    pending_previews: HashMap<String, (String, DateTime<Utc>)>,
}

impl ChatManager {
    /// Create a new chat manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all chats
    pub fn chats(&self) -> &[Chat] {
        &self.chats
    }

    /// Get mutable reference to chats
    pub fn chats_mut(&mut self) -> &mut Vec<Chat> {
        &mut self.chats
    }

    /// Get the selected chat JID
    pub fn selected(&self) -> Option<&Jid> {
        self.selected_chat.as_ref()
    }

    /// Select a chat by JID
    pub fn select(&mut self, jid: Jid) -> Option<&Chat> {
        self.selected_chat = Some(jid);
        self.get(&jid)
    }

    /// Get a chat by JID
    pub fn get(&self, jid: &Jid) -> Option<&Chat> {
        self.chats.iter().find(|c| &c.jid == jid)
    }

    /// Get a mutable chat by JID
    pub fn get_mut(&mut self, jid: &Jid) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|c| &c.jid == jid)
    }

    /// Clear the selection
    pub fn clear_selection(&mut self) {
        self.selected_chat = None;
    }

    /// Update or insert a chat from RPC data
    pub fn update_from_rpc(&mut self, chat: rpc::Chat) -> bool {
        let chat: Chat = chat.into();
        
        if let Some(existing) = self.chats.iter_mut().find(|c| c.jid == chat.jid) {
            let was_updated = existing.last_activity != chat.last_activity
                || existing.unread_count != chat.unread_count
                || existing.last_message != chat.last_message;
            *existing = chat;
            was_updated
        } else {
            self.chats.push(chat);
            true
        }
    }

    /// Set all chats from RPC data
    pub fn set_from_rpc(&mut self, chats: Vec<rpc::Chat>) -> bool {
        let had_chats = !self.chats.is_empty();
        self.chats = chats.into_iter().map(Chat::from).collect();
        !had_chats && !self.chats.is_empty()
    }

    /// Update a contact name across all matching chats
    pub fn update_contact_name(&mut self, jid: &Jid, name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
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

        updated
    }

    /// Update the last message preview for a chat
    pub fn update_preview(&mut self, chat_jid: &Jid, preview: String, timestamp: DateTime<Utc>) {
        // Check if we should update (newer timestamp)
        if let Some(last_ts) = self.chat_preview_timestamps.get(&chat_jid.0) {
            if timestamp < *last_ts {
                return;
            }
        }

        self.chat_preview_timestamps
            .insert(chat_jid.0.clone(), timestamp);

        if let Some(chat) = self.chats.iter_mut().find(|c| &c.jid == chat_jid) {
            chat.last_message = Some(preview);
            chat.last_activity = Some(timestamp);
        } else {
            // Create a new chat entry if it doesn't exist
            self.chats.push(Chat {
                jid: chat_jid.clone(),
                name: chat_jid.display_label(),
                last_message: Some(preview),
                last_activity: Some(timestamp),
                unread_count: 0,
                is_pinned: false,
                is_group: chat_jid.is_group(),
                is_muted: false,
            });
        }
    }

    /// Sort chats by pin status and activity
    pub fn sort(&mut self) {
        self.chats.sort_by(|a, b| {
            // First by pinned status
            let pinned_cmp = b.is_pinned.cmp(&a.is_pinned);
            if pinned_cmp != std::cmp::Ordering::Equal {
                return pinned_cmp;
            }
            // Then by last activity
            b.last_activity.cmp(&a.last_activity)
        });
    }

    /// Get pinned chats
    pub fn pinned_chats(&self) -> Vec<&Chat> {
        self.chats.iter().filter(|c| c.is_pinned).collect()
    }

    /// Get unpinned chats
    pub fn unpinned_chats(&self) -> Vec<&Chat> {
        self.chats.iter().filter(|c| !c.is_pinned).collect()
    }

    /// Get unread count for all chats
    pub fn total_unread_count(&self) -> u32 {
        self.chats.iter().map(|c| c.unread_count).sum()
    }

    /// Mark a chat as read
    pub fn mark_as_read(&mut self, jid: &Jid) -> bool {
        if let Some(chat) = self.chats.iter_mut().find(|c| &c.jid == jid) {
            let was_unread = chat.unread_count > 0;
            chat.unread_count = 0;
            was_unread
        } else {
            false
        }
    }

    /// Pin a chat
    pub fn pin_chat(&mut self, jid: &Jid) -> bool {
        if let Some(chat) = self.chats.iter_mut().find(|c| &c.jid == jid) {
            if !chat.is_pinned {
                chat.is_pinned = true;
                self.sort();
                return true;
            }
        }
        false
    }

    /// Unpin a chat
    pub fn unpin_chat(&mut self, jid: &Jid) -> bool {
        if let Some(chat) = self.chats.iter_mut().find(|c| &c.jid == jid) {
            if chat.is_pinned {
                chat.is_pinned = false;
                self.sort();
                return true;
            }
        }
        false
    }

    /// Mute a chat
    pub fn mute_chat(&mut self, jid: &Jid) -> bool {
        if let Some(chat) = self.chats.iter_mut().find(|c| &c.jid == jid) {
            if !chat.is_muted {
                chat.is_muted = true;
                return true;
            }
        }
        false
    }

    /// Unmute a chat
    pub fn unmute_chat(&mut self, jid: &Jid) -> bool {
        if let Some(chat) = self.chats.iter_mut().find(|c| &c.jid == jid) {
            if chat.is_muted {
                chat.is_muted = false;
                return true;
            }
        }
        false
    }

    /// Remove a chat
    pub fn remove_chat(&mut self, jid: &Jid) -> bool {
        let len_before = self.chats.len();
        self.chats.retain(|c| &c.jid != jid);
        self.chat_preview_timestamps.remove(&jid.0);
        self.chats.len() < len_before
    }

    /// Search chats by name
    pub fn search_chats(&self, query: &str) -> Vec<&Chat> {
        let query_lower = query.to_lowercase();
        self.chats
            .iter()
            .filter(|c| c.name.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Get chat count
    pub fn len(&self) -> usize {
        self.chats.len()
    }

    /// Check if there are no chats
    pub fn is_empty(&self) -> bool {
        self.chats.is_empty()
    }

    /// Clear all chats (for logout)
    pub fn clear(&mut self) {
        self.chats.clear();
        self.selected_chat = None;
        self.chat_preview_timestamps.clear();
        self.pending_previews.clear();
    }

    /// Check if we can show the chats view based on connection state
    pub fn can_show_chats_view(&self, connection: &crate::model::connection::ConnectionState) -> bool {
        !matches!(
            connection,
            crate::model::connection::ConnectionState::WaitingForQr { .. }
                | crate::model::connection::ConnectionState::WaitingForPairCode { .. }
                | crate::model::connection::ConnectionState::LoggedOut
        )
    }

    /// Get the appropriate view state based on chats and connection
    pub fn get_view_state(
        &self,
        connection: &crate::model::connection::ConnectionState,
    ) -> ViewState {
        if !self.chats.is_empty() && self.can_show_chats_view(connection) {
            ViewState::Chats
        } else {
            ViewState::Pairing
        }
    }
}
