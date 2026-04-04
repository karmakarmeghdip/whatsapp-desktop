//! WhatsApp API integration module
//!
//! This module provides a clean abstraction over the whatsapp-rust library,
//! integrating it with iced's subscription system for reactive UI updates.

mod client;
mod events;
pub mod storage;
mod types;

pub use client::{Connection, WhatsAppCommand};
pub use events::WhatsAppEvent;
pub use types::*;
