pub mod sidebar;
pub mod chat;

use iced::widget::row;
use iced::{Element, Task};
use crate::core::types::{Chat, ChatMessage, Message};

pub struct State {
    chats: Vec<Chat>,
    selected_chat: Option<usize>,
    messages: Vec<ChatMessage>,
    input_value: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            chats: vec![
                Chat { id: 1, name: "Alice".to_string(), last_message: "Hello there!".to_string() },
                Chat { id: 2, name: "Bob".to_string(), last_message: "How are you?".to_string() },
                Chat { id: 3, name: "Charlie".to_string(), last_message: "See you later".to_string() },
            ],
            selected_chat: None,
            messages: vec![
                ChatMessage { is_me: false, content: "Hello!".to_string() },
                ChatMessage { is_me: true, content: "Hi! How are you?".to_string() },
            ],
            input_value: String::new(),
        }
    }
}

impl State {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectChat(id) => {
                self.selected_chat = Some(id);
                Task::none()
            }
            Message::InputChanged(val) => {
                self.input_value = val;
                Task::none()
            }
            Message::SendMessage => {
                if !self.input_value.is_empty() {
                    self.messages.push(ChatMessage {
                        is_me: true,
                        content: self.input_value.clone(),
                    });
                    self.input_value.clear();
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = sidebar::view(&self.chats, self.selected_chat);
        
        let chat_area = if let Some(chat_id) = self.selected_chat {
            let chat_name = self.chats.iter().find(|c| c.id == chat_id).map(|c| c.name.as_str()).unwrap_or("");
            chat::view(&chat_name, &self.messages, &self.input_value)
        } else {
            chat::empty_view()
        };

        row![
            sidebar,
            chat_area
        ]
        .into()
    }
}
