//! Sidebar component showing the chat list

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};
use crate::core::types::{Chat, Message};
use crate::whatsapp::Jid;

/// Render the sidebar with chat list
pub fn view<'a>(chats: &'a [Chat], selected_chat: Option<&'a Jid>) -> Element<'a, Message> {
    let mut chat_list = column![].spacing(5).padding(10);

    // Sort chats: pinned first, then by last message
    let mut sorted_chats: Vec<_> = chats.iter().collect();
    sorted_chats.sort_by(|a, b| {
        b.is_pinned.cmp(&a.is_pinned)
    });

    for chat in sorted_chats {
        let is_selected = selected_chat.map(|s| s == &chat.jid).unwrap_or(false);
        let jid = chat.jid.clone();

        // Unread badge
        let unread_badge = if chat.unread_count > 0 {
            container(
                text(chat.unread_count.to_string())
                    .size(12)
            )
            .padding([2, 8])
            .style(|theme: &iced::Theme| {
                let mut style = container::primary(theme);
                style.border.radius = 10.0.into();
                style
            })
        } else {
            container(Space::new())
        };

        // Pin indicator
        let pin_indicator = if chat.is_pinned {
            text("📌").size(12)
        } else {
            text("").size(12)
        };

        let chat_button = button(
            row![
                column![
                    row![
                        text(&chat.name).size(16),
                        Space::new().width(Length::Fill),
                        pin_indicator,
                    ].align_y(iced::Alignment::Center),
                    text(&chat.last_message)
                        .size(13)
                        .style(|theme: &iced::Theme| {
                            text::Style {
                                color: Some(theme.palette().background.strongest.color),
                            }
                        }),
                ]
                .spacing(4)
                .width(Length::Fill),
                unread_badge,
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center)
        )
        .width(Length::Fill)
        .padding(12)
        .style(move |theme: &iced::Theme, status: iced::widget::button::Status| {
            let mut style = if is_selected {
                button::secondary(theme, status)
            } else {
                button::text(theme, status)
            };
            style.border = iced::border::Border {
                radius: 10.0.into(),
                ..style.border
            };
            style
        })
        .on_press(Message::SelectChat(jid));

        chat_list = chat_list.push(chat_button);
    }

    // Empty state
    if chats.is_empty() {
        chat_list = chat_list.push(
            container(
                column![
                    text("No chats yet").size(16),
                    text("Your conversations will appear here").size(13),
                ]
                .spacing(5)
                .align_x(iced::Alignment::Center)
            )
            .width(Length::Fill)
            .padding(20)
        );
    }

    container(
        column![
            // Header
            container(
                row![
                    text("Chats").size(22),
                    Space::new().width(Length::Fill),
                    // Settings button could go here
                ]
            )
            .padding(15)
            .width(Length::Fill),
            // Chat list
            scrollable(chat_list).height(Length::Fill)
        ]
    )
    .width(Length::Fixed(320.0))
    .height(Length::Fill)
    .style(|theme: &iced::Theme| {
        let mut style = iced::widget::container::bordered_box(theme);
        style.border.radius = 0.0.into();
        style
    })
    .into()
}