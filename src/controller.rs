//! Application Controller
//!
//! The controller handles all application messages and updates the model accordingly.
//! It is the bridge between user interactions (from views) and external events 
//! (from WhatsApp service) and the model state.

use iced::Task;
use crate::model::{AppState, ConnectionState, MessageStatus, ViewState};
use crate::whatsapp::{self, Jid, WhatsAppEvent};

/// Application message enum - all possible events that can update the model
#[derive(Debug, Clone)]
pub enum Message {
    // --- User interactions (from views) ---
    
    /// User selected a chat from the sidebar
    SelectChat(Jid),
    /// User typed in the message input
    InputChanged(String),
    /// User pressed send button or Enter
    SendMessage,
    /// User wants to go to settings
    ShowSettings,
    /// User wants to return to chat list
    BackToChats,

    // --- WhatsApp service events ---
    
    /// Event from the WhatsApp service
    WhatsApp(WhatsAppEvent),
}

/// Process a message and update the model, returning any follow-up tasks
pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        // User interactions
        Message::SelectChat(jid) => {
            state.select_chat(jid);
            Task::none()
        }

        Message::InputChanged(value) => {
            state.set_input(value);
            Task::none()
        }

        Message::SendMessage => {
            if let Some((jid, text)) = state.take_message_to_send() {
                // Add pending message for immediate UI feedback
                state.add_pending_message(&jid, text.clone());
                
                // TODO: Send via WhatsApp connection
                // Return a task that sends the message through the WhatsApp client
                log::info!("Sending message to {}: {}", jid.0, text);
            }
            Task::none()
        }

        Message::ShowSettings => {
            state.view = ViewState::Settings;
            Task::none()
        }

        Message::BackToChats => {
            state.view = ViewState::Chats;
            Task::none()
        }

        // WhatsApp service events
        Message::WhatsApp(event) => handle_whatsapp_event(state, event),
    }
}

/// Handle events from the WhatsApp service
fn handle_whatsapp_event(state: &mut AppState, event: WhatsAppEvent) -> Task<Message> {
    match event {
        WhatsAppEvent::ConnectionStateChanged(wa_state) => {
            log::info!("Connection state: {:?}", wa_state);
            let connection_state = convert_connection_state(wa_state);
            state.set_connection_state(connection_state);
        }

        WhatsAppEvent::QrCodeReceived { qr_code } => {
            log::debug!("QR code received");
            state.qr_code = Some(qr_code);
        }

        WhatsAppEvent::Connected => {
            log::info!("Connected to WhatsApp");
        }

        WhatsAppEvent::Disconnected => {
            log::warn!("Disconnected from WhatsApp");
        }

        WhatsAppEvent::LoggedOut => {
            log::warn!("Logged out from WhatsApp");
        }

        WhatsAppEvent::MessageReceived(msg) => {
            log::debug!("Message received: {:?}", msg.id);
            state.add_message(msg);
        }

        WhatsAppEvent::MessageSent { message_id, chat_jid } => {
            log::debug!("Message sent: {} to {}", message_id, chat_jid);
            state.update_message_status(&message_id, MessageStatus::Sent);
        }

        WhatsAppEvent::MessageStatusUpdated { message_id, status, .. } => {
            log::debug!("Message {} status: {:?}", message_id, status);
            state.update_message_status(&message_id, status.into());
        }

        WhatsAppEvent::ChatsUpdated(chats) => {
            log::debug!("Chats updated: {} chats", chats.len());
            state.set_chats(chats);
        }

        WhatsAppEvent::ChatUpdated(chat) => {
            state.update_chat(chat);
        }

        WhatsAppEvent::TypingIndicator { chat_jid, sender_jid, state: typing_state } => {
            log::trace!("{} typing in {}: {:?}", sender_jid, chat_jid, typing_state);
            state.set_typing(chat_jid, sender_jid, typing_state);
        }

        WhatsAppEvent::PresenceUpdated(presence) => {
            log::trace!("Presence: {} online={}", presence.jid, presence.is_online);
            // TODO: Store presence in model if needed
        }

        WhatsAppEvent::HistorySyncProgress { current, total } => {
            log::info!("History sync: {}/{}", current, total);
        }

        WhatsAppEvent::HistorySyncCompleted => {
            log::info!("History sync completed");
        }

        WhatsAppEvent::Error(error) => {
            log::error!("WhatsApp error: {}", error);
            state.set_error(error);
        }

        WhatsAppEvent::PairCodeReceived { .. } => {
            // Handle pair code if needed
        }
    }

    Task::none()
}

/// Convert WhatsApp connection state to model connection state
fn convert_connection_state(wa_state: whatsapp::ConnectionState) -> ConnectionState {
    match wa_state {
        whatsapp::ConnectionState::Disconnected => ConnectionState::Disconnected,
        whatsapp::ConnectionState::Connecting => ConnectionState::Connecting,
        whatsapp::ConnectionState::WaitingForQr { qr_code } => {
            ConnectionState::WaitingForQr { qr_code }
        }
        whatsapp::ConnectionState::WaitingForPairCode { code } => {
            ConnectionState::WaitingForPairCode { code }
        }
        whatsapp::ConnectionState::Connected => ConnectionState::Connected,
        whatsapp::ConnectionState::Reconnecting => ConnectionState::Reconnecting,
        whatsapp::ConnectionState::LoggedOut => ConnectionState::LoggedOut,
    }
}
