use iced::{Element, Task, Theme};
use crate::ui;
use crate::core::types::Message;

pub struct WhatsApp {
    state: ui::State,
}

impl WhatsApp {
    pub fn title(&self) -> String {
        String::from("WhatsApp Desktop")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        self.state.update(message)
    }

    pub fn view(&self) -> Element<'_, Message> {
        self.state.view()
    }

    pub fn theme(&self) -> Theme {
        Theme::CatppuccinMocha
    }
}

impl Default for WhatsApp {
    fn default() -> Self {
        Self {
            state: ui::State::default(),
        }
    }
}
