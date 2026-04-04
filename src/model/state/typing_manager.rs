//! Typing Manager
//!
//! Manages typing indicators for chats.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::rpc::Jid;

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

impl Default for TypingState {
    fn default() -> Self {
        TypingState::Idle
    }
}

/// Individual typing indicator entry
#[derive(Debug, Clone)]
struct TypingEntry {
    state: TypingState,
    updated_at: DateTime<Utc>,
}

/// Manages typing indicators across all chats
#[derive(Debug, Default)]
pub struct TypingManager {
    indicators: HashMap<String, HashMap<String, TypingEntry>>,
    expiry_seconds: i64,
}

impl TypingManager {
    /// Create a new typing manager with default expiry
    pub fn new() -> Self {
        Self {
            indicators: HashMap::new(),
            expiry_seconds: 8,
        }
    }

    /// Create with custom expiry time
    pub fn with_expiry(expiry_seconds: i64) -> Self {
        Self {
            indicators: HashMap::new(),
            expiry_seconds,
        }
    }

    /// Set typing state for a user in a chat
    pub fn set_typing(&mut self, chat_jid: &Jid, sender_jid: &Jid, state: TypingState) {
        self.indicators
            .entry(chat_jid.0.clone())
            .or_default()
            .insert(
                sender_jid.0.clone(),
                TypingEntry {
                    state,
                    updated_at: Utc::now(),
                },
            );
    }

    /// Get the active typing state for a chat
    /// Returns the most relevant state (Recording > Typing > Idle)
    pub fn get_state(&self, chat_jid: &Jid) -> TypingState {
        self.indicators
            .get(&chat_jid.0)
            .and_then(|indicators| {
                // Find the most active non-idle state
                let mut has_typing = false;
                
                for entry in indicators.values() {
                    match entry.state {
                        TypingState::Recording => return Some(TypingState::Recording),
                        TypingState::Typing => has_typing = true,
                        TypingState::Idle => {}
                    }
                }
                
                if has_typing {
                    Some(TypingState::Typing)
                } else {
                    None
                }
            })
            .unwrap_or(TypingState::Idle)
    }

    /// Check if anyone is typing in a chat
    pub fn is_typing(&self, chat_jid: &Jid) -> bool {
        matches!(self.get_state(chat_jid), TypingState::Typing | TypingState::Recording)
    }

    /// Check if anyone is recording audio in a chat
    pub fn is_recording(&self, chat_jid: &Jid) -> bool {
        matches!(self.get_state(chat_jid), TypingState::Recording)
    }

    /// Get all typing users in a chat
    pub fn get_typing_users(&self, chat_jid: &Jid) -> Vec<(String, TypingState)> {
        self.indicators
            .get(&chat_jid.0)
            .map(|indicators| {
                indicators
                    .iter()
                    .filter(|(_, entry)| entry.state != TypingState::Idle)
                    .map(|(user, entry)| (user.clone(), entry.state))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clear typing state for a specific user
    pub fn clear_user(&mut self, chat_jid: &Jid, sender_jid: &Jid) {
        if let Some(indicators) = self.indicators.get_mut(&chat_jid.0) {
            indicators.remove(&sender_jid.0);
        }
    }

    /// Clear all typing state for a chat
    pub fn clear_chat(&mut self, chat_jid: &Jid) {
        self.indicators.remove(&chat_jid.0);
    }

    /// Clean up expired typing indicators
    pub fn cleanup(&mut self) {
        let now = Utc::now();
        let expiry = chrono::Duration::seconds(self.expiry_seconds);

        self.indicators.retain(|_, indicators| {
            indicators.retain(|_, entry| {
                entry.state != TypingState::Idle && (now - entry.updated_at) < expiry
            });
            !indicators.is_empty()
        });
    }

    /// Get all active chats with typing indicators
    pub fn active_chats(&self) -> Vec<String> {
        self.indicators.keys().cloned().collect()
    }

    /// Check if there are any active typing indicators
    pub fn has_active_indicators(&self) -> bool {
        !self.indicators.is_empty()
    }

    /// Clear all typing indicators
    pub fn clear_all(&mut self) {
        self.indicators.clear();
    }

    /// Get the number of active typing sessions
    pub fn active_count(&self) -> usize {
        self.indicators.values().map(|v| v.len()).sum()
    }

    /// Set expiry duration for typing indicators
    pub fn set_expiry(&mut self, seconds: i64) {
        self.expiry_seconds = seconds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typing_state_default() {
        let state: TypingState = Default::default();
        assert_eq!(state, TypingState::Idle);
    }

    #[test]
    fn test_set_and_get_typing() {
        let mut manager = TypingManager::new();
        let chat = Jid("123@s.whatsapp.net".to_string());
        let user = Jid("456@s.whatsapp.net".to_string());

        manager.set_typing(&chat, &user, TypingState::Typing);
        assert!(manager.is_typing(&chat));
        assert!(!manager.is_recording(&chat));
    }

    #[test]
    fn test_recording_priority() {
        let mut manager = TypingManager::new();
        let chat = Jid("123@s.whatsapp.net".to_string());
        let user1 = Jid("456@s.whatsapp.net".to_string());
        let user2 = Jid("789@s.whatsapp.net".to_string());

        manager.set_typing(&chat, &user1, TypingState::Typing);
        manager.set_typing(&chat, &user2, TypingState::Recording);
        
        assert!(manager.is_recording(&chat));
        assert_eq!(manager.get_state(&chat), TypingState::Recording);
    }

    #[test]
    fn test_cleanup() {
        let mut manager = TypingManager::with_expiry(0); // Immediate expiry
        let chat = Jid("123@s.whatsapp.net".to_string());
        let user = Jid("456@s.whatsapp.net".to_string());

        manager.set_typing(&chat, &user, TypingState::Typing);
        assert!(manager.has_active_indicators());

        manager.cleanup();
        assert!(!manager.has_active_indicators());
    }

    #[test]
    fn test_clear_chat() {
        let mut manager = TypingManager::new();
        let chat1 = Jid("123@s.whatsapp.net".to_string());
        let chat2 = Jid("789@g.us".to_string());
        let user = Jid("456@s.whatsapp.net".to_string());

        manager.set_typing(&chat1, &user, TypingState::Typing);
        manager.set_typing(&chat2, &user, TypingState::Typing);

        manager.clear_chat(&chat1);
        
        assert!(!manager.is_typing(&chat1));
        assert!(manager.is_typing(&chat2));
    }
}
