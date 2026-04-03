mod app;
mod core;
mod ui;
mod whatsapp;

use app::WhatsApp;

pub fn main() -> iced::Result {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{:<5}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    log::info!("Starting WhatsApp Desktop");

    iced::application(WhatsApp::new, WhatsApp::update, WhatsApp::view)
        .title(WhatsApp::title)
        .theme(WhatsApp::theme)
        .subscription(WhatsApp::subscription)
        .run()
}
