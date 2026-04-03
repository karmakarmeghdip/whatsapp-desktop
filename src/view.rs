//! Application Views
//!
//! Views are pure functions that take the model state and return iced Elements.
//! They should contain no business logic - only presentation concerns.
//! 
//! Views emit Messages that the controller handles to update the model.

pub(crate) mod chat;
mod loading;
mod main_view;
mod pairing;
mod settings;
mod sidebar;

// Re-export the main render function and components
pub use main_view::render;
pub use loading::loading;
pub use pairing::pairing;
pub use settings::settings;
