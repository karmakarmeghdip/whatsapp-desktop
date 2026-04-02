use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};
use crate::core::types::{ChatMessage, Message};

pub fn view<'a>(chat_name: &'a str, messages: &'a [ChatMessage], input_value: &'a str) -> Element<'a, Message> {
    let header = container(
        text(chat_name).size(20)
    )
    .padding(20)
    .width(Length::Fill)
    .style(|theme: &iced::Theme| {
        let mut style = iced::widget::container::bordered_box(theme);
        style.border.radius = 0.0.into();
        style
    });

    let mut message_list = column![].spacing(10).padding(20);
    for msg in messages {
        let is_me = msg.is_me;
        let content_text = msg.content.clone();
        
        let msg_content = container(text(content_text))
            .padding(15)
            .style(move |theme: &iced::Theme| {
                let mut style = if is_me { container::primary(theme) } else { container::rounded_box(theme) };
                style.border = iced::border::Border {
                    radius: 7.5.into(),
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

    let messages_scroll = scrollable(message_list).height(Length::Fill);

    let input_area = container(
        row![
            text_input("Type a message", input_value)
                .on_input(Message::InputChanged)
                .on_submit(Message::SendMessage)
                .padding(15)
                .width(Length::Fill)
                .style(move |theme: &iced::Theme, status: iced::widget::text_input::Status| {
                    let mut style = iced::widget::text_input::default(theme, status);
                    style.border = iced::border::Border {
                        radius: 26.0.into(),
                        ..style.border
                    };
                    style
                }),
            button(
                iced::widget::svg(iced::widget::svg::Handle::from_memory(format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                    icondata::LuSend.data
                ).into_bytes()))
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0))
                .style(|_theme: &iced::Theme, _status| iced::widget::svg::Style {
                    color: Some(iced::Color::WHITE),
                })
            )
            .on_press(Message::SendMessage)
            .padding(15)
            .style(move |theme: &iced::Theme, status: iced::widget::button::Status| {
                let mut style = button::primary(theme, status);
                style.border = iced::border::Border {
                    radius: 25.0.into(),
                    ..style.border
                };
                style
            })
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    )
    .padding(20)
    .width(Length::Fill)
    .style(|theme: &iced::Theme| {
        let mut style = iced::widget::container::bordered_box(theme);
        style.border.radius = 0.0.into();
        style
    });

    column![
        header,
        messages_scroll,
        input_area
    ]
    .width(Length::Fill)
    .into()
}

pub fn empty_view<'a>() -> Element<'a, Message> {
    container(
        text("Select a chat to start messaging")
            .size(24)
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
