//! Application Controller
//!
//! The controller handles all application messages and updates the model accordingly.
//! It is the bridge between user interactions (from views) and external events 
//! (from WhatsApp service) and the model state.

use iced::Task;
use iced::widget::{operation, scrollable};
use crate::model::{AppState, ConnectionState, MessageStatus, ViewState};
use crate::whatsapp::{self, Jid, WhatsAppEvent, WhatsAppCommand};

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
    /// User scrolled message viewport
    MessageListScrolled(scrollable::Viewport),
    /// Internal timer tick for periodic cleanup
    Tick,

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
            operation::snap_to(chat_scroll_id(), scrollable::RelativeOffset::END)
        }

        Message::InputChanged(value) => {
            state.set_input(value);
            Task::none()
        }

        Message::SendMessage => {
            if let Some((jid, text)) = state.take_message_to_send() {
                // Add pending message for immediate UI feedback
                let local_id = state.add_pending_message(&jid, text.clone());
                
                // Send via WhatsApp connection
                if let Some(ref mut connection) = state.whatsapp {
                    log::info!("Sending message to {}: {}", jid.0, text);
                    connection.send(WhatsAppCommand::SendMessage { 
                        local_id,
                        chat_jid: jid, 
                        text 
                    });
                } else {
                    log::warn!("Cannot send message: not connected to WhatsApp");
                    state.update_specific_message_status(&jid, &local_id, MessageStatus::Failed);
                    state.set_error("Not connected to WhatsApp".to_string());
                }
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

        Message::MessageListScrolled(viewport) => {
            if state.consume_scroll_ignore_flag() {
                return Task::none();
            }

            if viewport.relative_offset().y <= 0.02
                && let Some((chat_jid, oldest_msg_id, oldest_msg_from_me, oldest_msg_timestamp_ms)) =
                    state.selected_chat_history_cursor()
                && state.start_older_history_request_if_allowed(&chat_jid, &oldest_msg_id)
                && let Some(ref mut connection) = state.whatsapp
            {
                connection.send(WhatsAppCommand::FetchHistory {
                    chat_jid,
                    oldest_msg_id,
                    oldest_msg_from_me,
                    oldest_msg_timestamp_ms,
                    count: 100,
                });
            }
            Task::none()
        }

        Message::Tick => {
            state.cleanup_temporary_state();
            Task::none()
        }

        // WhatsApp service events
        Message::WhatsApp(event) => handle_whatsapp_event(state, event),
    }
}

fn chat_scroll_id() -> &'static str {
    crate::view::chat::messages_scroll_id()
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

        WhatsAppEvent::Connected(connection) => {
            log::info!("Connected to WhatsApp");
            state.set_whatsapp_connection(connection);
        }

        WhatsAppEvent::Disconnected => {
            log::warn!("Disconnected from WhatsApp");
            state.clear_whatsapp_connection();
        }

        WhatsAppEvent::LoggedOut => {
            log::warn!("Logged out from WhatsApp");
            state.clear_whatsapp_connection();
        }

        WhatsAppEvent::MessageReceived(msg) => {
            log::debug!("Message received: {:?}", msg.id);
            state.add_message(msg);
        }

        WhatsAppEvent::MessageSent { local_id, message_id, chat_jid } => {
            log::debug!("Message sent: {} -> {} ({})", local_id, message_id, chat_jid);
            state.resolve_pending_message_id(&chat_jid, &local_id, &message_id);
            state.update_message_status(&message_id, MessageStatus::Sent);
        }

        WhatsAppEvent::MessageSendFailed { local_id, chat_jid, error } => {
            log::warn!("Message send failed: {} ({}) - {}", local_id, chat_jid, error);
            state.update_specific_message_status(&chat_jid, &local_id, MessageStatus::Failed);
            state.set_error(format!("Failed to send message: {}", error));
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

        WhatsAppEvent::ContactNameUpdated { jid, name } => {
            state.update_contact_name(&jid, &name);
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
            state.set_sync_progress(current, total);
        }

        WhatsAppEvent::HistorySyncCompleted => {
            log::info!("History sync completed");
            state.finish_sync();
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
