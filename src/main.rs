mod app;
mod core;
mod ui;

use app::WhatsApp;

pub fn main() -> iced::Result {
    iced::application(WhatsApp::default, WhatsApp::update, WhatsApp::view)
        .title(WhatsApp::title)
        .theme(WhatsApp::theme)
        .run()
}
