//! Application Model
//!
//! The model contains the entire application state. It is the single source of truth
//! for all data in the application. The model is read by views to render the UI
//! and modified by the controller in response to user actions and external events.

mod chat;
mod connection;
mod state;

// Re-export main types
pub use chat::{Chat, ChatMessage, MessageStatus};
pub use connection::{ConnectionState, ViewState};
pub use state::AppState;
