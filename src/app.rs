//! Application entry point and iced integration
//!
//! This module wires together the Model, View, and Controller with iced's
//! application framework. It serves as the glue between MVC and iced.

use iced::{Element, Subscription, Task, Theme};
use crate::controller::{self, Message};
use crate::model::AppState;
use crate::view;
use crate::whatsapp;

/// Main application struct - holds the model and provides iced integration
pub struct App {
    /// The application model (single source of truth)
    state: AppState,
}

impl App {
    /// Create a new application instance
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                state: AppState::new(),
            },
            Task::none(),
        )
    }

    /// Window title - derived from model state
    pub fn title(&self) -> String {
        let suffix = match &self.state.connection {
            crate::model::ConnectionState::Connected => "",
            crate::model::ConnectionState::Connecting => " - Connecting...",
            crate::model::ConnectionState::WaitingForQr { .. } => " - Scan QR Code",
            crate::model::ConnectionState::WaitingForPairCode { .. } => " - Enter Code",
            crate::model::ConnectionState::Reconnecting => " - Reconnecting...",
            crate::model::ConnectionState::LoggedOut => " - Logged Out",
            crate::model::ConnectionState::Disconnected => " - Disconnected",
        };
        format!("WhatsApp Desktop{}", suffix)
    }

    /// Handle messages - delegates to controller
    pub fn update(&mut self, message: Message) -> Task<Message> {
        controller::update(&mut self.state, message)
    }

    /// Render the view - delegates to view module
    pub fn view(&self) -> Element<'_, Message> {
        view::render(&self.state)
    }

    /// Application theme
    pub fn theme(&self) -> Theme {
        Theme::CatppuccinMocha
    }

    /// Background subscriptions - WhatsApp connection
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(whatsapp::connect).map(Message::WhatsApp)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().0
    }
}
