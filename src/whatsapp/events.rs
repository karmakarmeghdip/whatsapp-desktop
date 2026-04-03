//! WhatsApp events for the UI layer

use super::client::Connection;
use super::types::*;

/// Events emitted by the WhatsApp client for UI updates
#[derive(Debug, Clone)]
pub enum WhatsAppEvent {
    /// Connection state changed
    ConnectionStateChanged(ConnectionState),

    /// QR code received for pairing
    QrCodeReceived {
        qr_code: String,
    },

    /// Pair code received (alternative to QR)
    PairCodeReceived {
        code: String,
    },

    /// Successfully paired and connected (includes connection handle for sending)
    Connected(Connection),

    /// Disconnected from WhatsApp
    Disconnected,

    /// Logged out (session invalidated)
    LoggedOut,

    /// New message received
    MessageReceived(ChatMessage),

    /// Message sent successfully
    MessageSent {
        message_id: String,
        chat_jid: Jid,
    },

    /// Message status updated (delivered, read, etc.)
    MessageStatusUpdated {
        message_id: String,
        chat_jid: Jid,
        status: MessageStatus,
    },

    /// Chat list updated
    ChatsUpdated(Vec<Chat>),

    /// Single chat updated
    ChatUpdated(Chat),

    /// Typing indicator received
    TypingIndicator {
        chat_jid: Jid,
        sender_jid: Jid,
        state: TypingState,
    },

    /// Presence update (online/offline)
    PresenceUpdated(Presence),

    /// History sync progress
    HistorySyncProgress {
        current: u32,
        total: u32,
    },

    /// History sync completed
    HistorySyncCompleted,

    /// Error occurred
    Error(String),
}
