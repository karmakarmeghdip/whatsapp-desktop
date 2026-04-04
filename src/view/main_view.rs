//! Main view - composes sidebar and chat area

use iced::widget::row;
use iced::Element;
use crate::controller::Message;
use crate::model::{AppState, ViewState};
use super::{loading, pairing, sidebar, chat};

/// Render the appropriate view based on application state
pub fn render(state: &AppState) -> Element<'_, Message> {
    match state.view {
        ViewState::Loading => loading::loading(),
        ViewState::Pairing => pairing::pairing(state.qr_code_data.as_ref()),
        ViewState::Chats => chats_view(state),
    }
}

/// Render the main chats view with sidebar and chat area
fn chats_view(state: &AppState) -> Element<'_, Message> {
    let sidebar = sidebar::sidebar(
        &state.chats,
        state.selected_chat.as_ref(),
        if state.sync_in_progress { state.sync_progress } else { None },
    );

    let chat_area = if let Some(chat) = state.selected_chat() {
        chat::chat_view(
            &chat.name,
            state.selected_messages(),
            &state.input_value,
            state.selected_typing_state(),
            state.loading_older_messages,
        )
    } else {
        chat::empty_view()
    };

    row![sidebar, chat_area].into()
}
