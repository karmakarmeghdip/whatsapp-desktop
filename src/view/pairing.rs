//! Pairing view - QR code display for WhatsApp linking

use iced::widget::{center, column, container, text, Space};
use iced::{Alignment, Element, Length};
use crate::controller::Message;

/// Pairing view with QR code display
pub fn pairing<'a>(qr_code: Option<&'a str>) -> Element<'a, Message> {
    let content = if let Some(qr) = qr_code {
        column![
            text("Scan QR Code").size(28),
            Space::new().height(20),
            container(
                // Display QR code as text (TODO: render as actual QR image)
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
            text("Waiting for QR Code...").size(24),
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
