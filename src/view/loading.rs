//! Loading view - displayed while connecting

use iced::widget::{center, column, text, Space};
use iced::{Alignment, Element, Length};
use crate::controller::Message;

/// Loading view while connecting to WhatsApp
pub fn loading<'a>() -> Element<'a, Message> {
    center(
        column![
            text("Connecting to WhatsApp...").size(24),
            Space::new().height(20),
            text("Please wait while we establish a connection").size(16),
        ]
        .align_x(Alignment::Center)
        .spacing(10),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
