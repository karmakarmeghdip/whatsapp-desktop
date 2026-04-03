//! WhatsApp client connection and subscription management
//!
//! This module provides an iced-compatible subscription for WhatsApp connectivity,
//! following the patterns from iced's websocket example.

use std::sync::Arc;

use futures::channel::mpsc;
use futures::stream::StreamExt;
use iced::task::{Never, Sipper, sipper};

use super::events::WhatsAppEvent;
use super::types::*;

// Re-export types from the correct modules
use wacore::types::presence::{
    ChatPresence as WaChatPresence,
    ChatPresenceMedia as WaChatPresenceMedia,
    ReceiptType,
};
use whatsapp_rust::ChatStateType;

/// Command to send to the WhatsApp client
#[derive(Debug, Clone)]
pub enum WhatsAppCommand {
    /// Send a text message
    SendMessage { chat_jid: Jid, text: String },
    /// Send typing indicator
    SendTyping { chat_jid: Jid, typing: bool },
    /// Mark chat as read
    MarkAsRead { chat_jid: Jid },
    /// Disconnect from WhatsApp
    Disconnect,
}

/// Connection handle for sending commands to WhatsApp
#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<WhatsAppCommand>);

impl Connection {
    /// Send a command to the WhatsApp client
    pub fn send(&mut self, command: WhatsAppCommand) {
        self.0.try_send(command).expect("Send command to WhatsApp client");
    }

    /// Send a text message
    pub fn send_message(&mut self, chat_jid: Jid, text: String) {
        self.send(WhatsAppCommand::SendMessage { chat_jid, text });
    }

    /// Send typing indicator
    pub fn send_typing(&mut self, chat_jid: Jid, typing: bool) {
        self.send(WhatsAppCommand::SendTyping { chat_jid, typing });
    }

    /// Mark a chat as read
    pub fn mark_as_read(&mut self, chat_jid: Jid) {
        self.send(WhatsAppCommand::MarkAsRead { chat_jid });
    }
}

/// Creates a subscription that connects to WhatsApp and emits events
///
/// This follows the iced sipper pattern for long-running async operations
/// that need to both emit events and receive commands.
pub fn connect() -> impl Sipper<Never, WhatsAppEvent> {
    sipper(async move |mut output| {
        use whatsapp_rust::bot::Bot;
        use whatsapp_rust::TokioRuntime;
        use whatsapp_rust::store::SqliteStore;
        use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
        use whatsapp_rust_ureq_http_client::UreqHttpClient;
        use wacore::types::events::Event;
        use wacore::proto_helpers::MessageExt;

        loop {
            // Attempt to connect to WhatsApp
            output.send(WhatsAppEvent::ConnectionStateChanged(ConnectionState::Connecting)).await;

            // Initialize storage backend
            let db_path = dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("whatsapp-desktop")
                .join("whatsapp.db");

            // Ensure directory exists
            if let Some(parent) = db_path.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    log::error!("Failed to create data directory: {}", e);
                    output.send(WhatsAppEvent::Error(format!("Failed to create data directory: {}", e))).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            }

            log::info!("Using database: {}", db_path.display());
            
            let backend = match SqliteStore::new(db_path.to_string_lossy().as_ref()).await {
                Ok(store) => Arc::new(store),
                Err(e) => {
                    log::error!("Failed to initialize storage: {}", e);
                    output.send(WhatsAppEvent::Error(format!("Failed to initialize storage: {}", e))).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            };

            // Create a channel for the event handler to send events back
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
            let (_command_tx, mut command_rx) = mpsc::channel::<WhatsAppCommand>(100);

            // Build the bot with event forwarding
            let event_tx_clone = event_tx.clone();
            let bot_result = Bot::builder()
                .with_backend(backend)
                .with_transport_factory(TokioWebSocketTransportFactory::new())
                .with_http_client(UreqHttpClient::new())
                .with_runtime(TokioRuntime)
                .on_event(move |event, _client| {
                    let tx = event_tx_clone.clone();
                    async move {
                        let _ = tx.send(event);
                    }
                })
                .build()
                .await;

            let mut bot = match bot_result {
                Ok(bot) => bot,
                Err(e) => {
                    log::error!("Failed to build bot: {}", e);
                    output.send(WhatsAppEvent::Error(format!("Failed to build bot: {}", e))).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            };

            // Get the client handle for sending messages
            let client = bot.client();

            // Start the bot in a separate task
            let mut bot_handle = tokio::spawn(async move {
                let _ = bot.run().await;
            });

            // Process events from the bot
            loop {
                tokio::select! {
                    // Handle incoming WhatsApp events
                    Some(event) = event_rx.recv() => {
                        match event {
                            Event::PairingQrCode { code, .. } => {
                                log::info!("QR code received for pairing");
                                output.send(WhatsAppEvent::QrCodeReceived {
                                    qr_code: code.clone(),
                                }).await;
                                output.send(WhatsAppEvent::ConnectionStateChanged(
                                    ConnectionState::WaitingForQr { qr_code: code },
                                )).await;
                            }
                            Event::PairingCode { code, .. } => {
                                log::info!("Pair code received: {}", code);
                                output.send(WhatsAppEvent::PairCodeReceived {
                                    code: code.clone(),
                                }).await;
                                output.send(WhatsAppEvent::ConnectionStateChanged(
                                    ConnectionState::WaitingForPairCode { code },
                                )).await;
                            }
                            Event::Connected(_) => {
                                log::info!("Connected to WhatsApp");
                                output.send(WhatsAppEvent::Connected).await;
                                output.send(WhatsAppEvent::ConnectionStateChanged(
                                    ConnectionState::Connected,
                                )).await;
                            }
                            Event::Disconnected(_) => {
                                log::warn!("Disconnected from WhatsApp");
                                output.send(WhatsAppEvent::Disconnected).await;
                                output.send(WhatsAppEvent::ConnectionStateChanged(
                                    ConnectionState::Reconnecting,
                                )).await;
                            }
                            Event::LoggedOut(_) => {
                                log::warn!("Logged out from WhatsApp");
                                output.send(WhatsAppEvent::LoggedOut).await;
                                output.send(WhatsAppEvent::ConnectionStateChanged(
                                    ConnectionState::LoggedOut,
                                )).await;
                                break;
                            }
                            Event::Message(msg, info) => {
                                // Convert WhatsApp message to our type
                                let content = if let Some(text) = msg.text_content() {
                                    MessageContent::Text(text.to_string())
                                } else if let Some(img) = &msg.image_message {
                                    MessageContent::Image {
                                        caption: img.caption.clone(),
                                        url: img.url.clone(),
                                        thumbnail: img.jpeg_thumbnail.clone(),
                                    }
                                } else if let Some(vid) = &msg.video_message {
                                    MessageContent::Video {
                                        caption: vid.caption.clone(),
                                        url: vid.url.clone(),
                                        thumbnail: vid.jpeg_thumbnail.clone(),
                                    }
                                } else if let Some(aud) = &msg.audio_message {
                                    MessageContent::Audio {
                                        url: aud.url.clone(),
                                        duration_secs: aud.seconds.unwrap_or(0),
                                        is_voice_note: aud.ptt.unwrap_or(false),
                                    }
                                } else if let Some(doc) = &msg.document_message {
                                    MessageContent::Document {
                                        filename: doc.file_name.clone().unwrap_or_default(),
                                        url: doc.url.clone(),
                                        mime_type: doc.mimetype.clone(),
                                    }
                                } else if let Some(sticker) = &msg.sticker_message {
                                    MessageContent::Sticker {
                                        url: sticker.url.clone(),
                                    }
                                } else if let Some(loc) = &msg.location_message {
                                    MessageContent::Location {
                                        latitude: loc.degrees_latitude.unwrap_or(0.0),
                                        longitude: loc.degrees_longitude.unwrap_or(0.0),
                                        name: loc.name.clone(),
                                    }
                                } else {
                                    MessageContent::Unknown
                                };

                                let chat_message = ChatMessage {
                                    id: info.id.clone(),
                                    sender: Jid::new(info.source.sender.to_string()),
                                    chat: Jid::new(info.source.chat.to_string()),
                                    content,
                                    timestamp: info.timestamp,
                                    is_from_me: info.source.is_from_me,
                                    status: MessageStatus::Delivered,
                                    quoted_message: None,
                                };

                                output.send(WhatsAppEvent::MessageReceived(chat_message)).await;
                            }
                            Event::Receipt(receipt) => {
                                let status = match receipt.r#type {
                                    ReceiptType::Read |
                                    ReceiptType::ReadSelf => MessageStatus::Read,
                                    ReceiptType::Delivered => MessageStatus::Delivered,
                                    _ => MessageStatus::Sent,
                                };

                                for msg_id in receipt.message_ids {
                                    output.send(WhatsAppEvent::MessageStatusUpdated {
                                        message_id: msg_id.clone(),
                                        chat_jid: Jid::new(receipt.source.chat.to_string()),
                                        status,
                                    }).await;
                                }
                            }
                            Event::ChatPresence(update) => {
                                let state = match (update.state, update.media) {
                                    (WaChatPresence::Composing,
                                     WaChatPresenceMedia::Audio) => TypingState::Recording,
                                    (WaChatPresence::Composing, _) => TypingState::Typing,
                                    (WaChatPresence::Paused, _) => TypingState::Idle,
                                };

                                output.send(WhatsAppEvent::TypingIndicator {
                                    chat_jid: Jid::new(update.source.chat.to_string()),
                                    sender_jid: Jid::new(update.source.sender.to_string()),
                                    state,
                                }).await;
                            }
                            Event::Presence(update) => {
                                output.send(WhatsAppEvent::PresenceUpdated(Presence {
                                    jid: Jid::new(update.from.to_string()),
                                    is_online: !update.unavailable,
                                    last_seen: update.last_seen,
                                })).await;
                            }
                            Event::OfflineSyncPreview(preview) => {
                                output.send(WhatsAppEvent::HistorySyncProgress {
                                    current: 0,
                                    total: preview.total as u32,
                                }).await;
                            }
                            Event::OfflineSyncCompleted(_sync) => {
                                output.send(WhatsAppEvent::HistorySyncCompleted).await;
                            }
                            _ => {
                                // Handle other events as needed
                            }
                        }
                    }

                    // Handle commands from UI
                    Some(command) = command_rx.next() => {
                        match command {
                            WhatsAppCommand::SendMessage { chat_jid, text } => {
                                use waproto::whatsapp as wa;
                                use wacore_binary::jid::Jid as WaJid;

                                let message = wa::Message {
                                    conversation: Some(text),
                                    ..Default::default()
                                };

                                // Parse JID string to WhatsApp JID type
                                // Format: user@server or user:device@server
                                let jid_str = &chat_jid.0;
                                if let Some((user, server)) = jid_str.split_once('@') {
                                    let wa_jid = WaJid::new(user, server);
                                    if let Err(e) = client.send_message(wa_jid, message).await {
                                        output.send(WhatsAppEvent::Error(format!("Failed to send message: {}", e))).await;
                                    }
                                }
                            }
                            WhatsAppCommand::SendTyping { chat_jid, typing } => {
                                use wacore_binary::jid::Jid as WaJid;

                                let jid_str = &chat_jid.0;
                                if let Some((user, server)) = jid_str.split_once('@') {
                                    let wa_jid = WaJid::new(user, server);
                                    let state = if typing {
                                        ChatStateType::Composing
                                    } else {
                                        ChatStateType::Paused
                                    };
                                    let _ = client.chatstate().send(&wa_jid, state).await;
                                }
                            }
                            WhatsAppCommand::MarkAsRead { chat_jid: _ } => {
                                // Mark messages as read - would need message IDs in practice
                            }
                            WhatsAppCommand::Disconnect => {
                                break;
                            }
                        }
                    }

                    // Bot task ended
                    _ = &mut bot_handle => {
                        break;
                    }
                }
            }

            // Connection ended, wait before reconnecting
            output.send(WhatsAppEvent::Disconnected).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    })
}
