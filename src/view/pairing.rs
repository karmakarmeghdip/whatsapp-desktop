//! Pairing view - QR code display for WhatsApp linking

use iced::widget::{center, column, container, qr_code, text, Space};
use iced::{Alignment, Element, Length};
use crate::controller::Message;

/// Pairing view with QR code display
pub fn pairing<'a>(qr_data: Option<&'a qr_code::Data>) -> Element<'a, Message> {
    let content = if let Some(data) = qr_data {
        column![
            text("Scan QR Code").size(28),
            Space::new().height(20),
            container(
                qr_code(data).cell_size(5)
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
