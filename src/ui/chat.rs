//! Chat view component

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};
use crate::core::types::{ChatMessage, Message};
use crate::whatsapp::{MessageStatus, TypingState};

/// Render the chat view
pub fn view<'a>(
    chat_name: &'a str,
    messages: &'a [ChatMessage],
    input_value: &'a str,
    typing: Option<TypingState>,
) -> Element<'a, Message> {
    // Header
    let header = container(
        row![
            text(chat_name).size(18),
            Space::new().width(Length::Fill),
            // Online status could go here
        ]
        .align_y(Alignment::Center)
    )
    .padding(15)
    .width(Length::Fill)
    .style(|theme: &iced::Theme| {
        let mut style = iced::widget::container::bordered_box(theme);
        style.border.radius = 0.0.into();
        style
    });

    // Messages
    let mut message_list = column![].spacing(8).padding(15);

    for msg in messages {
        let is_me = msg.is_me;

        // Status indicator for sent messages
        let status_text = if is_me {
            match msg.status {
                MessageStatus::Pending => " ⏳",
                MessageStatus::Sent => " ✓",
                MessageStatus::Delivered => " ✓✓",
                MessageStatus::Read => " ✓✓",
                MessageStatus::Failed => " ❌",
            }
        } else {
            ""
        };

        let content_text = format!("{}{}", msg.content, status_text);

        let msg_content = container(text(content_text))
            .padding(12)
            .max_width(500)
            .style(move |theme: &iced::Theme| {
                let mut style = if is_me {
                    container::primary(theme)
                } else {
                    container::rounded_box(theme)
                };
                style.border = iced::border::Border {
                    radius: 12.0.into(),
                    ..style.border
                };
                style
            });

        let msg_row = if is_me {
            row![Space::new().width(Length::Fill), msg_content]
        } else {
            row![msg_content, Space::new().width(Length::Fill)]
        };

        message_list = message_list.push(msg_row);
    }

    // Typing indicator
    if let Some(typing_state) = typing {
        let typing_text = match typing_state {
            TypingState::Typing => "typing...",
            TypingState::Recording => "recording audio...",
            TypingState::Idle => "",
        };

        if !typing_text.is_empty() {
            message_list = message_list.push(
                container(
                    text(typing_text)
                        .size(13)
                        .style(|theme: &iced::Theme| {
                            text::Style {
                                color: Some(theme.palette().background.strongest.color),
                            }
                        })
                )
                .padding(8)
            );
        }
    }

    let messages_scroll = scrollable(message_list).height(Length::Fill);

    // Input area
    let input_area = container(
        row![
            text_input("Type a message", input_value)
                .on_input(Message::InputChanged)
                .on_submit(Message::SendMessage)
                .padding(12)
                .width(Length::Fill)
                .style(|theme: &iced::Theme, status: iced::widget::text_input::Status| {
                    let mut style = iced::widget::text_input::default(theme, status);
                    style.border = iced::border::Border {
                        radius: 20.0.into(),
                        ..style.border
                    };
                    style
                }),
            button(
                iced::widget::svg(iced::widget::svg::Handle::from_memory(format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                    icondata::LuSend.data
                ).into_bytes()))
                .width(Length::Fixed(18.0))
                .height(Length::Fixed(18.0))
                .style(|_theme: &iced::Theme, _status| iced::widget::svg::Style {
                    color: Some(iced::Color::WHITE),
                })
            )
            .on_press(Message::SendMessage)
            .padding(12)
            .style(|theme: &iced::Theme, status: iced::widget::button::Status| {
                let mut style = button::primary(theme, status);
                style.border = iced::border::Border {
                    radius: 20.0.into(),
                    ..style.border
                };
                style
            })
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    )
    .padding(15)
    .width(Length::Fill)
    .style(|theme: &iced::Theme| {
        let mut style = iced::widget::container::bordered_box(theme);
        style.border.radius = 0.0.into();
        style
    });

    column![header, messages_scroll, input_area]
        .width(Length::Fill)
        .into()
}

/// Empty state when no chat is selected
pub fn empty_view<'a>() -> Element<'a, Message> {
    container(
        column![
            text("💬").size(64),
            Space::new().height(10),
            text("Select a chat to start messaging").size(18),
        ]
        .align_x(Alignment::Center)
        .spacing(10)
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
