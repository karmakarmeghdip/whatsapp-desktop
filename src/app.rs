//! Application entry point and iced integration
//!
//! This module wires together the Model, View, and Controller with iced's
//! application framework. It serves as the glue between MVC and iced.

use iced::{Element, Subscription, Task, Theme};
use crate::controller::{self, Message};
use crate::model::AppState;
use crate::rpc;
use crate::view;

/// Main application struct - holds the model and provides iced integration
pub struct App {
    state: AppState,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                state: AppState::new(),
            },
            Task::none(),
        )
    }

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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        controller::update(&mut self.state, message)
    }

    pub fn view(&self) -> Element<'_, Message> {
        view::render(&self.state)
    }

    pub fn theme(&self) -> Theme {
        Theme::CatppuccinMocha
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            rpc::client::subscription().map(Message::RpcNotification),
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::Tick),
        ])
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new().0
    }
}
