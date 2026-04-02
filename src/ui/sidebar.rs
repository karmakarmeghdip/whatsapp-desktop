use iced::widget::{button, column, container, scrollable, text};
use iced::{Element, Length};
use crate::core::types::{Chat, Message};

pub fn view<'a>(chats: &'a [Chat], selected_chat: Option<usize>) -> Element<'a, Message> {
    let mut chat_list = column![].spacing(5).padding(10);

    for chat in chats {
        let is_selected = selected_chat == Some(chat.id);
        
        let chat_button = button(
            column![
                text(&chat.name).size(18),
                text(&chat.last_message).size(14),
            ]
            .spacing(5)
        )
        .width(Length::Fill)
        .padding(15)
        .style(move |theme: &iced::Theme, status: iced::widget::button::Status| {
            let mut style = if is_selected {
                button::secondary(theme, status)
            } else {
                button::text(theme, status)
            };
            style.border = iced::border::Border {
                radius: 12.0.into(),
                ..style.border
            };
            style
        })
        .on_press(Message::SelectChat(chat.id));

        chat_list = chat_list.push(chat_button);
    }

    container(
        column![
            container(text("Chats").size(24)).padding(20).width(Length::Fill),
            scrollable(chat_list).height(Length::Fill)
        ]
    )
    .width(Length::Fixed(300.0))
    .height(Length::Fill)
    .style(container::bordered_box)
    .into()
}