//! WhatsApp client types and commands
//!
//! This module provides types and commands for WhatsApp integration.
//! The actual connection logic is in rpc/service.rs

use futures::channel::mpsc;

/// Command to send to the WhatsApp client
#[derive(Debug, Clone)]
pub enum WhatsAppCommand {
    /// Send a text message
    SendMessage {
        local_id: String,
        chat_jid: super::Jid,
        text: String,
    },
    /// Send typing indicator
    SendTyping { chat_jid: super::Jid, typing: bool },
    /// Mark chat as read
    MarkAsRead,
}

/// Connection handle for sending commands to WhatsApp
#[derive(Debug, Clone)]
pub struct Connection(pub mpsc::Sender<WhatsAppCommand>);

impl Connection {
    /// Send a command to the WhatsApp client
    pub fn send(&mut self, command: WhatsAppCommand) {
        self.0
            .try_send(command)
            .expect("Send command to WhatsApp client");
    }

    /// Send a text message with a specific local ID
    pub fn send_message_with_id(&mut self, local_id: String, chat_jid: super::Jid, text: String) {
        self.send(WhatsAppCommand::SendMessage {
            local_id,
            chat_jid,
            text,
        });
    }

    /// Send typing indicator
    pub fn send_typing(&mut self, chat_jid: super::Jid, typing: bool) {
        self.send(WhatsAppCommand::SendTyping { chat_jid, typing });
    }

    /// Mark a chat as read
    pub fn mark_as_read(&mut self, _chat_jid: super::Jid) {
        self.send(WhatsAppCommand::MarkAsRead);
    }
}
