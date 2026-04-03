//! Settings view

use iced::widget::{button, center, column, text, Space};
use iced::{Alignment, Element, Length};
use crate::controller::Message;

/// Settings view (placeholder)
pub fn settings<'a>() -> Element<'a, Message> {
    center(
        column![
            text("Settings").size(28),
            Space::new().height(20),
            text("Settings coming soon...").size(16),
            Space::new().height(20),
            button(text("Back to Chats"))
                .on_press(Message::BackToChats)
                .padding(15),
        ]
        .align_x(Alignment::Center)
        .spacing(10),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
