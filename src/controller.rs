//! Application Controller
//!
//! The controller handles all application messages and updates the model accordingly.
//! It is the bridge between user interactions (from views) and external events
//! (from WhatsApp service) and the model state.

use iced::Task;
use iced::widget::{operation, scrollable};
use crate::model::{AppState, ConnectionState, MessageStatus};
use crate::rpc::{self, RpcNotification, RpcRequest, Jid};

/// Application message enum - all possible events that can update the model
#[derive(Debug, Clone)]
pub enum Message {
    /// User selected a chat from the sidebar
    SelectChat(Jid),
    /// User typed in the message input
    InputChanged(String),
    /// User pressed send button or Enter
    SendMessage,
    /// User scrolled message viewport
    MessageListScrolled(scrollable::Viewport),
    /// Internal timer tick for periodic cleanup
    Tick,
    /// Event from the WhatsApp RPC service
    RpcNotification(RpcNotification),
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

                // Send via RPC client
                if let Some(ref mut client) = state.rpc_client {
                    log::info!("Sending message to {}: {}", jid.0, text);
                    client.send(RpcRequest::SendMessage {
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

        Message::MessageListScrolled(viewport) => {
            if state.consume_scroll_ignore_flag() {
                return Task::none();
            }

            // Track scroll position for auto-scroll behavior
            let _is_at_bottom = state.update_scroll_position(viewport.relative_offset().y);

            if viewport.relative_offset().y <= 0.02
                && let Some((chat_jid, oldest_msg_id, oldest_msg_from_me, oldest_msg_timestamp_ms)) =
                    state.selected_chat_history_cursor()
                && state.start_older_history_request_if_allowed(&chat_jid, &oldest_msg_id)
                && let Some(ref mut client) = state.rpc_client
            {
                client.send(RpcRequest::FetchHistory {
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

        // WhatsApp service events via RPC
        Message::RpcNotification(notification) => handle_rpc_notification(state, notification),
    }
}

fn chat_scroll_id() -> &'static str {
    crate::view::chat::messages_scroll_id()
}

/// Handle notifications from the WhatsApp RPC service
fn handle_rpc_notification(state: &mut AppState, notification: RpcNotification) -> Task<Message> {
    match notification {
        RpcNotification::ServiceReady => {
            log::info!("RPC service ready");
            if let Some(handle) = crate::rpc::get_rpc_client_handle() {
                state.set_rpc_client(handle);
            }
        }

        RpcNotification::ConnectionStateChanged(rpc_state) => {
            log::info!("Connection state: {:?}", rpc_state);
            let connection_state = convert_rpc_connection_state(rpc_state);
            state.set_connection_state(connection_state);
        }

        RpcNotification::QrCodeReceived { qr_code } => {
            log::debug!("QR code received");
            state.qr_code = Some(qr_code);
        }

        RpcNotification::Connected => {
            log::info!("Connected to WhatsApp via RPC");
            // The RPC client handle is already in state
        }

        RpcNotification::Disconnected => {
            log::warn!("Disconnected from WhatsApp via RPC");
            state.clear_rpc_client();
        }

        RpcNotification::LoggedOut => {
            log::warn!("Logged out from WhatsApp via RPC");
            state.clear_rpc_client();
        }

        RpcNotification::MessageReceived(msg) => {
            let should_scroll = state.should_auto_scroll();
            let is_for_selected = state.selected_chat.as_ref().map(|s| s.0 == msg.chat.0).unwrap_or(false);
            state.add_rpc_message(msg);
            if should_scroll && is_for_selected {
                return scroll_to_bottom();
            }
        }

        RpcNotification::MessageSent { local_id, message_id, chat_jid } => {
            let should_scroll = state.should_auto_scroll();
            state.resolve_pending_message_id(&chat_jid, &local_id, &message_id);
            state.update_message_status(&message_id, MessageStatus::Sent);
            if should_scroll {
                return scroll_to_bottom();
            }
        }

        RpcNotification::MessageSendFailed { local_id, chat_jid, error } => {
            log::warn!("Message send failed: {} ({}) - {}", local_id, chat_jid, error);
            state.update_specific_message_status(&chat_jid, &local_id, MessageStatus::Failed);
            state.set_error(format!("Failed to send message: {}", error));
        }

        RpcNotification::MessageStatusUpdated { message_id, status, .. } => {
            log::debug!("Message {} status: {:?}", message_id, status);
            state.update_message_status(&message_id, convert_rpc_message_status(status));
        }

        RpcNotification::ChatsUpdated(chats) => {
            log::debug!("Chats updated: {} chats", chats.len());
            state.set_chats_from_rpc(chats);
        }

        RpcNotification::ChatUpdated(chat) => {
            state.update_chat_from_rpc(chat);
        }

        RpcNotification::ContactNameUpdated { jid, name } => {
            state.update_contact_name(&jid, &name);
        }

        RpcNotification::TypingIndicator { chat_jid, sender_jid, state: typing_state } => {
            let should_scroll = state.should_auto_scroll();
            let is_for_selected = state.selected_chat.as_ref().map(|s| s.0 == chat_jid.0).unwrap_or(false);
            state.set_typing(chat_jid, sender_jid, convert_rpc_typing_state(typing_state));
            if should_scroll && is_for_selected {
                return scroll_to_bottom();
            }
        }

        RpcNotification::PresenceUpdated(presence) => {
            log::trace!("Presence: {} online={}", presence.jid, presence.is_online);
            // TODO: Store presence in model if needed
        }

        RpcNotification::HistorySyncProgress { current, total } => {
            log::info!("History sync: {}/{}", current, total);
            state.set_sync_progress(current, total);
        }

        RpcNotification::HistorySyncCompleted => {
            log::info!("History sync completed");
            state.finish_sync();
        }

        RpcNotification::Error(error) => {
            log::error!("WhatsApp RPC error: {}", error);
            state.set_error(error);
        }

        RpcNotification::PairCodeReceived { code } => {
            log::debug!("Pair code received: {}", code);
        }
    }

    Task::none()
}

/// Convert RPC connection state to model connection state
fn convert_rpc_connection_state(rpc_state: rpc::ConnectionState) -> ConnectionState {
    match rpc_state {
        rpc::ConnectionState::Disconnected => ConnectionState::Disconnected,
        rpc::ConnectionState::Connecting => ConnectionState::Connecting,
        rpc::ConnectionState::WaitingForQr { qr_code } => {
            ConnectionState::WaitingForQr { qr_code }
        }
        rpc::ConnectionState::WaitingForPairCode { code } => {
            ConnectionState::WaitingForPairCode { code }
        }
        rpc::ConnectionState::Connected => ConnectionState::Connected,
        rpc::ConnectionState::Reconnecting => ConnectionState::Reconnecting,
        rpc::ConnectionState::LoggedOut => ConnectionState::LoggedOut,
    }
}

/// Convert RPC message status to model message status
fn convert_rpc_message_status(rpc_status: rpc::MessageStatus) -> MessageStatus {
    match rpc_status {
        rpc::MessageStatus::Pending => MessageStatus::Pending,
        rpc::MessageStatus::Sent => MessageStatus::Sent,
        rpc::MessageStatus::Delivered => MessageStatus::Delivered,
        rpc::MessageStatus::Read => MessageStatus::Read,
        rpc::MessageStatus::Failed => MessageStatus::Failed,
    }
}

/// Scroll the message list to the bottom
fn scroll_to_bottom() -> Task<Message> {
    use iced::widget::{operation, scrollable};
    operation::snap_to(chat_scroll_id(), scrollable::RelativeOffset::END)
}

/// Convert RPC typing state to model typing state
fn convert_rpc_typing_state(rpc_state: rpc::TypingState) -> crate::model::TypingState {
    match rpc_state {
        rpc::TypingState::Idle => crate::model::TypingState::Idle,
        rpc::TypingState::Typing => crate::model::TypingState::Typing,
        rpc::TypingState::Recording => crate::model::TypingState::Recording,
    }
}
