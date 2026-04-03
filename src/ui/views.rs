//! View components for different application states

use iced::widget::{button, center, column, container, text, Space};
use iced::{Alignment, Element, Length};
use crate::core::types::Message;

/// Loading view while connecting
pub fn loading_view<'a>() -> Element<'a, Message> {
    center(
        column![
            text("🔄 Connecting to WhatsApp...").size(24),
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

/// Pairing view with QR code
pub fn pairing_view<'a>(qr_code: Option<&'a str>) -> Element<'a, Message> {
    let content = if let Some(qr) = qr_code {
        column![
            text("Scan QR Code").size(28),
            Space::new().height(20),
            container(
                // Display QR code as text (in a real app, you'd render it as an image)
                text(qr).size(8).font(iced::Font::MONOSPACE)
            )
            .padding(20)
            .style(|theme: &iced::Theme| {
                let mut style = container::rounded_box(theme);
                style.background = Some(iced::Color::WHITE.into());
                style
            }),
            Space::new().height(20),
            text("Open WhatsApp on your phone").size(16),
            text("Go to Settings → Linked Devices → Link a Device").size(14),
            text("Point your phone at this screen to scan the QR code").size(14),
        ]
        .align_x(Alignment::Center)
        .spacing(10)
    } else {
        column![
            text("📱 Waiting for QR Code...").size(24),
            Space::new().height(20),
            text("Please wait while we generate a QR code").size(16),
        ]
        .align_x(Alignment::Center)
        .spacing(10)
    };

    center(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Settings view (placeholder)
pub fn settings_view<'a>() -> Element<'a, Message> {
    center(
        column![
            text("⚙️ Settings").size(28),
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
