//! Main application struct and iced integration

use iced::{Element, Subscription, Task, Theme};
use crate::core::types::{Message, ViewState};
use crate::ui;
use crate::whatsapp::{self, WhatsAppEvent, ConnectionState};

/// Main application state
pub struct WhatsApp {
    /// UI state
    state: ui::State,
    /// Current view
    view_state: ViewState,
    /// Connection state
    connection_state: ConnectionState,
    /// QR code for pairing (if available)
    qr_code: Option<String>,
    /// Error message (if any)
    error: Option<String>,
}

impl WhatsApp {
    /// Create new application instance
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                state: ui::State::default(),
                view_state: ViewState::Loading,
                connection_state: ConnectionState::Disconnected,
                qr_code: None,
                error: None,
            },
            Task::none(),
        )
    }

    /// Window title
    pub fn title(&self) -> String {
        let suffix = match &self.connection_state {
            ConnectionState::Connected => "",
            ConnectionState::Connecting => " - Connecting...",
            ConnectionState::WaitingForQr { .. } => " - Scan QR Code",
            ConnectionState::WaitingForPairCode { .. } => " - Enter Code",
            ConnectionState::Reconnecting => " - Reconnecting...",
            ConnectionState::LoggedOut => " - Logged Out",
            ConnectionState::Disconnected => " - Disconnected",
        };
        format!("WhatsApp Desktop{}", suffix)
    }

    /// Handle messages
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WhatsApp(event) => self.handle_whatsapp_event(event),
            Message::SelectChat(jid) => {
                self.state.select_chat(jid);
                Task::none()
            }
            Message::InputChanged(value) => {
                self.state.set_input(value);
                Task::none()
            }
            Message::SendMessage => {
                self.state.send_message();
                Task::none()
            }
            Message::ShowSettings => {
                self.view_state = ViewState::Settings;
                Task::none()
            }
            Message::ShowPairing => {
                self.view_state = ViewState::Pairing;
                Task::none()
            }
            Message::BackToChats => {
                self.view_state = ViewState::Chats;
                Task::none()
            }
        }
    }

    /// Handle WhatsApp events
    fn handle_whatsapp_event(&mut self, event: WhatsAppEvent) -> Task<Message> {
        match event {
            WhatsAppEvent::ConnectionStateChanged(state) => {
                log::info!("Connection state: {:?}", state);
                self.connection_state = state.clone();

                // Update view state based on connection
                match state {
                    ConnectionState::Connected => {
                        self.view_state = ViewState::Chats;
                        self.qr_code = None;
                        self.error = None;
                    }
                    ConnectionState::WaitingForQr { qr_code } => {
                        self.view_state = ViewState::Pairing;
                        self.qr_code = Some(qr_code);
                    }
                    ConnectionState::WaitingForPairCode { .. } => {
                        self.view_state = ViewState::Pairing;
                    }
                    ConnectionState::LoggedOut => {
                        self.view_state = ViewState::Pairing;
                        self.qr_code = None;
                    }
                    ConnectionState::Disconnected => {
                        self.view_state = ViewState::Loading;
                    }
                    _ => {}
                }
            }
            WhatsAppEvent::QrCodeReceived { qr_code } => {
                log::debug!("QR code received");
                self.qr_code = Some(qr_code);
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
                self.state.add_message(msg);
            }
            WhatsAppEvent::MessageSent { message_id, chat_jid } => {
                log::debug!("Message sent: {} to {}", message_id, chat_jid);
            }
            WhatsAppEvent::MessageStatusUpdated { message_id, status, .. } => {
                log::debug!("Message {} status: {:?}", message_id, status);
                self.state.update_message_status(&message_id, status);
            }
            WhatsAppEvent::ChatsUpdated(chats) => {
                log::debug!("Chats updated: {} chats", chats.len());
                self.state.set_chats(chats);
            }
            WhatsAppEvent::ChatUpdated(chat) => {
                self.state.update_chat(chat);
            }
            WhatsAppEvent::TypingIndicator { chat_jid, sender_jid, state } => {
                log::trace!("{} typing in {}: {:?}", sender_jid, chat_jid, state);
                self.state.set_typing(chat_jid, sender_jid, state);
            }
            WhatsAppEvent::PresenceUpdated(presence) => {
                log::trace!("Presence: {} online={}", presence.jid, presence.is_online);
            }
            WhatsAppEvent::HistorySyncProgress { current, total } => {
                log::info!("History sync: {}/{}", current, total);
            }
            WhatsAppEvent::HistorySyncCompleted => {
                log::info!("History sync completed");
            }
            WhatsAppEvent::Error(error) => {
                log::error!("WhatsApp error: {}", error);
                self.error = Some(error);
            }
            _ => {}
        }
        Task::none()
    }

    /// Render the view
    pub fn view(&self) -> Element<'_, Message> {
        match self.view_state {
            ViewState::Loading => ui::views::loading_view(),
            ViewState::Pairing => ui::views::pairing_view(self.qr_code.as_deref()),
            ViewState::Chats => self.state.view(),
            ViewState::Settings => ui::views::settings_view(),
        }
    }

    /// Application theme
    pub fn theme(&self) -> Theme {
        Theme::CatppuccinMocha
    }

    /// Subscriptions for background tasks
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(whatsapp::connect).map(Message::WhatsApp)
    }
}

impl Default for WhatsApp {
    fn default() -> Self {
        Self::new().0
    }
}
