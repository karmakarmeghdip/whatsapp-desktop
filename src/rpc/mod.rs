//! RPC Layer for WhatsApp Service Communication
//!
//! This module provides a thin RPC-like layer using mpsc channels.
//! All types are serializable (serde) to support future process separation.
//! The goal is to allow the WhatsApp service to eventually run as a separate
//! daemon process communicating over sockets via JSON-RPC.

use std::sync::OnceLock;
use serde::{Deserialize, Serialize};

pub mod client;
pub mod service;
pub mod types;

pub use client::RpcClientHandle;
pub use types::*;

/// Global storage for the RPC client handle so it can be accessed from the controller
static RPC_CLIENT_HANDLE: OnceLock<RpcClientHandle> = OnceLock::new();

/// Store the RPC client handle globally
pub fn set_rpc_client_handle(handle: RpcClientHandle) {
    let _ = RPC_CLIENT_HANDLE.set(handle);
}

/// Get a clone of the RPC client handle if it's been set
pub fn get_rpc_client_handle() -> Option<RpcClientHandle> {
    RPC_CLIENT_HANDLE.get().cloned()
}

/// RPC Request - commands from UI to service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcRequest {
    SendMessage {
        local_id: String,
        chat_jid: Jid,
        text: String,
    },
    SendTyping {
        chat_jid: Jid,
        typing: bool,
    },
    MarkAsRead {
        chat_jid: Jid,
    },
    FetchHistory {
        chat_jid: Jid,
        oldest_msg_id: String,
        oldest_msg_from_me: bool,
        oldest_msg_timestamp_ms: i64,
        count: i32,
    },
    Disconnect,
}

/// RPC Notification - events from service to UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcNotification {
    /// Service is ready and RPC client handle is available
    ServiceReady,
    ConnectionStateChanged(ConnectionState),
    QrCodeReceived {
        qr_code: String,
    },
    PairCodeReceived {
        code: String,
    },
    Connected,
    Disconnected,
    LoggedOut,
    MessageReceived(ChatMessage),
    MessageSent {
        local_id: String,
        message_id: String,
        chat_jid: Jid,
    },
    MessageSendFailed {
        local_id: String,
        chat_jid: Jid,
        error: String,
    },
    MessageStatusUpdated {
        message_id: String,
        chat_jid: Jid,
        status: MessageStatus,
    },
    ChatsUpdated(Vec<Chat>),
    ChatUpdated(Chat),
    ContactNameUpdated {
        jid: Jid,
        name: String,
    },
    TypingIndicator {
        chat_jid: Jid,
        sender_jid: Jid,
        state: TypingState,
    },
    PresenceUpdated(Presence),
    HistorySyncProgress {
        current: u32,
        total: u32,
    },
    HistorySyncCompleted,
    Error(String),
}
