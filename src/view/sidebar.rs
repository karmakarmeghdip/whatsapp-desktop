//! Sidebar component - displays the list of chats

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Element, Length};
use crate::controller::Message;
use crate::model::{Chat, Jid};

/// Render the sidebar with chat list
pub fn sidebar<'a>(
    chats: &'a [Chat],
    selected_chat: Option<&'a Jid>,
    sync_progress: Option<(u32, u32)>,
) -> Element<'a, Message> {
    let mut chat_list = column![];
    chat_list = chat_list.spacing(5);
    chat_list = chat_list.padding(10);

    let mut sorted_chats: Vec<_> = chats.iter().collect();
    sorted_chats.sort_by(|a, b| {
        // First sort by pinned status (pinned chats come first)
        match b.is_pinned.cmp(&a.is_pinned) {
            std::cmp::Ordering::Equal => {
                // Then sort by last activity (most recent first)
                b.last_activity.cmp(&a.last_activity)
            }
            other => other
        }
    });

    for chat in sorted_chats {
        let is_selected = selected_chat.map(|s| s == &chat.jid).unwrap_or(false);
        let jid = chat.jid.clone();

        let unread_badge = if chat.unread_count > 0 {
            container(text(chat.unread_count.to_string()).size(12))
                .padding([2, 8])
                .style(|theme: &iced::Theme| {
                    let mut style = container::primary(theme);
                    style.border.radius = 10.0.into();
                    style
                })
        } else {
            container(Space::new())
        };

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
                    ]
                    .align_y(iced::Alignment::Center),
                    text(&chat.last_message)
                        .size(13)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(theme.palette().background.strongest.color),
                        }),
                ]
                .spacing(4)
                .width(Length::Fill),
                unread_badge,
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
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

    if chats.is_empty() {
        chat_list = chat_list.push(
            container(
                column![
                    text("No chats yet").size(16),
                    text("Your conversations will appear here").size(13),
                ]
                .spacing(5)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding(20),
        );
    }

    let sync_banner = if let Some((current, total)) = sync_progress {
        let label = if total > 0 {
            format!("Syncing chats: {}/{}", current, total)
        } else {
            format!("Syncing chats: {}", current)
        };

        container(text(label).size(13)).padding([6, 12]).width(Length::Fill)
    } else {
        container(Space::new()).padding(0)
    };

    container(
        column![
            container(
                row![
                    text("Chats").size(22),
                    Space::new().width(Length::Fill),
                ]
            )
            .padding(15)
            .width(Length::Fill),
            sync_banner,
            scrollable(chat_list).height(Length::Fill)
        ],
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
