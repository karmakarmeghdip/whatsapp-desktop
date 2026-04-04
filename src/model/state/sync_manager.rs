//! Sync Manager
//!
//! Manages history sync state and progress tracking.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};

use crate::rpc::Jid;

/// Sync state for a specific operation
#[derive(Debug, Clone)]
pub struct SyncState {
    pub current: u32,
    pub total: u32,
    pub started_at: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub completed: bool,
}

impl SyncState {
    /// Create a new sync state
    pub fn new(current: u32, total: u32) -> Self {
        let now = Utc::now();
        Self {
            current,
            total: total.max(current),
            started_at: now,
            last_update: now,
            completed: false,
        }
    }

    /// Update progress
    pub fn update(&mut self, current: u32, total: u32) {
        self.current = current;
        self.total = total.max(current);
        self.last_update = Utc::now();
    }

    /// Mark as completed
    pub fn complete(&mut self) {
        self.current = self.total;
        self.completed = true;
        self.last_update = Utc::now();
    }

    /// Get progress percentage (0-100)
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f32 / self.total as f32) * 100.0
        }
    }

    /// Check if sync is stale (no updates for a while)
    pub fn is_stale(&self, threshold_seconds: i64) -> bool {
        (Utc::now() - self.last_update) > Duration::seconds(threshold_seconds)
    }

    /// Time elapsed since start
    pub fn elapsed(&self) -> Duration {
        Utc::now() - self.started_at
    }

    /// Estimated time remaining (simple linear projection)
    pub fn estimated_remaining(&self) -> Option<Duration> {
        if self.completed || self.current == 0 {
            return None;
        }

        let elapsed = self.elapsed();
        let rate = elapsed.num_milliseconds() as f64 / self.current as f64;
        let remaining_items = (self.total - self.current) as f64;
        let remaining_ms = (rate * remaining_items) as i64;

        Some(Duration::milliseconds(remaining_ms))
    }
}

/// Manages sync operations and progress tracking
#[derive(Debug, Default)]
pub struct SyncManager {
    history_sync: Option<SyncState>,
    history_request_state: HashMap<String, (String, DateTime<Utc>)>,
    loading_older_messages: bool,
    older_loading_updated_at: Option<DateTime<Utc>>,
    sync_expiry_seconds: i64,
    loading_expiry_seconds: i64,
}

impl SyncManager {
    /// Create a new sync manager with default expiry times
    pub fn new() -> Self {
        Self {
            history_sync: None,
            history_request_state: HashMap::new(),
            loading_older_messages: false,
            older_loading_updated_at: None,
            sync_expiry_seconds: 8,
            loading_expiry_seconds: 6,
        }
    }

    /// Check if a history sync is in progress
    pub fn is_syncing(&self) -> bool {
        self.history_sync.as_ref().map_or(false, |s| !s.completed)
    }

    /// Get current sync progress
    pub fn progress(&self) -> Option<(u32, u32)> {
        self.history_sync.as_ref().map(|s| (s.current, s.total))
    }

    /// Get sync percentage (0-100)
    pub fn percentage(&self) -> Option<f32> {
        self.history_sync.as_ref().map(|s| s.percentage())
    }

    /// Start or update history sync
    pub fn set_progress(&mut self, current: u32, total: u32) {
        match &mut self.history_sync {
            Some(state) => state.update(current, total),
            None => {
                self.history_sync = Some(SyncState::new(current, total));
            }
        }
    }

    /// Mark history sync as completed
    pub fn finish_sync(&mut self) {
        if let Some(state) = &mut self.history_sync {
            state.complete();
        }
    }

    /// Check if sync has stalled
    pub fn is_sync_stalled(&self) -> bool {
        self.history_sync
            .as_ref()
            .map_or(false, |s| !s.completed && s.is_stale(self.sync_expiry_seconds))
    }

    /// Clear sync state
    pub fn clear_sync(&mut self) {
        self.history_sync = None;
    }

    /// Check if we can request older history for a chat
    pub fn can_request_history(&self, chat_jid: &Jid, oldest_msg_id: &str) -> bool {
        let now = Utc::now();

        if let Some((last_oldest_id, requested_at)) = self.history_request_state.get(&chat_jid.0) {
            let is_same_cursor = last_oldest_id == oldest_msg_id;
            let is_recent = (now - *requested_at) < Duration::seconds(3);
            if is_same_cursor && is_recent {
                return false;
            }
        }

        true
    }

    /// Record a history request
    pub fn record_history_request(&mut self, chat_jid: &Jid, oldest_msg_id: &str) {
        self.history_request_state.insert(
            chat_jid.0.clone(),
            (oldest_msg_id.to_string(), Utc::now()),
        );
        self.loading_older_messages = true;
        self.older_loading_updated_at = Some(Utc::now());
    }

    /// Check if older messages are being loaded
    pub fn is_loading_older(&self) -> bool {
        self.loading_older_messages
    }

    /// Check if loading has stalled
    pub fn is_loading_stalled(&self) -> bool {
        if !self.loading_older_messages {
            return false;
        }

        self.older_loading_updated_at
            .map_or(false, |last| (Utc::now() - last) > Duration::seconds(self.loading_expiry_seconds))
    }

    /// Mark loading as complete
    pub fn finish_loading(&mut self) {
        self.loading_older_messages = false;
        self.older_loading_updated_at = None;
    }

    /// Clean up stale sync states
    pub fn cleanup(&mut self) {
        let now = Utc::now();

        // Clear completed/stale sync
        if let Some(ref state) = self.history_sync {
            if state.completed || state.is_stale(self.sync_expiry_seconds) {
                self.history_sync = None;
            }
        }

        // Clear stale loading state
        if self.loading_older_messages
            && self
                .older_loading_updated_at
                .map_or(false, |last| (now - last) > Duration::seconds(self.loading_expiry_seconds))
        {
            self.loading_older_messages = false;
            self.older_loading_updated_at = None;
        }

        // Clean up old history request records (older than 1 minute)
        self.history_request_state
            .retain(|_, (_, time)| (now - *time) < Duration::seconds(60));
    }

    /// Get sync state details
    pub fn sync_state(&self) -> Option<&SyncState> {
        self.history_sync.as_ref()
    }

    /// Set expiry times
    pub fn set_expiry(&mut self, sync_seconds: i64, loading_seconds: i64) {
        self.sync_expiry_seconds = sync_seconds;
        self.loading_expiry_seconds = loading_seconds;
    }

    /// Reset all sync state (for logout)
    pub fn reset(&mut self) {
        self.history_sync = None;
        self.history_request_state.clear();
        self.loading_older_messages = false;
        self.older_loading_updated_at = None;
    }

    /// Get the last history request time for a chat
    pub fn last_history_request(&self, chat_jid: &Jid) -> Option<DateTime<Utc>> {
        self.history_request_state
            .get(&chat_jid.0)
            .map(|(_, time)| *time)
    }

    /// Get estimated time remaining for sync
    pub fn estimated_time_remaining(&self) -> Option<Duration> {
        self.history_sync.as_ref().and_then(|s| s.estimated_remaining())
    }

    /// Format remaining time as human readable string
    pub fn formatted_remaining(&self) -> Option<String> {
        self.estimated_time_remaining().map(|duration| {
            let secs = duration.num_seconds();
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m", secs / 60)
            } else {
                format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_percentage() {
        let state = SyncState::new(50, 100);
        assert_eq!(state.percentage(), 50.0);

        let state = SyncState::new(0, 100);
        assert_eq!(state.percentage(), 0.0);

        let state = SyncState::new(100, 100);
        assert_eq!(state.percentage(), 100.0);
    }

    #[test]
    fn test_sync_progress() {
        let mut manager = SyncManager::new();
        assert!(!manager.is_syncing());

        manager.set_progress(50, 100);
        assert!(manager.is_syncing());
        assert_eq!(manager.progress(), Some((50, 100)));

        manager.finish_sync();
        assert!(!manager.is_syncing());
    }

    #[test]
    fn test_history_request_dedup() {
        let mut manager = SyncManager::new();
        let chat = Jid("123@s.whatsapp.net".to_string());

        assert!(manager.can_request_history(&chat, "msg1"));
        manager.record_history_request(&chat, "msg1");
        
        // Should not allow same request immediately
        assert!(!manager.can_request_history(&chat, "msg1"));
        
        // Should allow different message
        assert!(manager.can_request_history(&chat, "msg2"));
    }

    #[test]
    fn test_cleanup() {
        let mut manager = SyncManager::with_expiry(0, 0); // Immediate expiry
        let chat = Jid("123@s.whatsapp.net".to_string());

        manager.set_progress(50, 100);
        manager.record_history_request(&chat, "msg1");

        assert!(manager.is_syncing());
        assert!(manager.is_loading_older());

        manager.cleanup();

        assert!(!manager.is_syncing());
        assert!(!manager.is_loading_older());
    }
}
