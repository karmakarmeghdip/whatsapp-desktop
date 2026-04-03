//! WhatsApp client connection and subscription management
//!
//! This module provides an iced-compatible subscription for WhatsApp connectivity,
//! following the patterns from iced's websocket example.

use std::sync::Arc;

use futures::channel::mpsc;
use futures::stream::StreamExt;
use iced::task::{Never, Sipper, sipper};
use chrono::{DateTime, Utc};

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
    SendMessage { local_id: String, chat_jid: Jid, text: String },
    /// Send typing indicator
    SendTyping { chat_jid: Jid, typing: bool },
    /// Mark chat as read
    MarkAsRead { chat_jid: Jid },
    /// Fetch older history for a chat via PDO
    FetchHistory {
        chat_jid: Jid,
        oldest_msg_id: String,
        oldest_msg_from_me: bool,
        oldest_msg_timestamp_ms: i64,
        count: i32,
    },
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
        self.send(WhatsAppCommand::SendMessage {
            local_id: format!("manual_{}", Utc::now().timestamp_millis()),
            chat_jid,
            text,
        });
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
            let (command_tx, mut command_rx) = mpsc::channel::<WhatsAppCommand>(100);
            
            // Create the connection handle that will be passed to the UI
            let connection = Connection(command_tx);

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

            // Start the bot and get the handle
            log::info!("Starting bot...");
            let bot_handle_result = bot.run().await;
            let bot_join_handle = match bot_handle_result {
                Ok(handle) => {
                    log::info!("Bot started successfully, waiting for events...");
                    handle
                }
                Err(e) => {
                    log::error!("Failed to start bot: {}", e);
                    output.send(WhatsAppEvent::Error(format!("Failed to start bot: {}", e))).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    continue;
                }
            };

            // Run the bot handle in a separate task (this keeps the connection alive)
            let mut bot_task = tokio::spawn(async move {
                let _ = bot_join_handle.await;
            });

            let mut sync_current: u32 = 0;
            let mut sync_total_hint: Option<u32> = None;

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
                                output.send(WhatsAppEvent::Connected(connection.clone())).await;
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
                                let content = parse_message_content(msg.as_ref());

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

                                if !info.push_name.is_empty() {
                                    output
                                        .send(WhatsAppEvent::ContactNameUpdated {
                                            jid: Jid::new(info.source.sender.to_string()),
                                            name: info.push_name.clone(),
                                        })
                                        .await;
                                }

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
                                sync_current = 0;
                                sync_total_hint = Some(preview.total.max(0) as u32);
                                output.send(WhatsAppEvent::HistorySyncProgress {
                                    current: 0,
                                    total: preview.total as u32,
                                }).await;
                            }
                            Event::OfflineSyncCompleted(_sync) => {
                                sync_total_hint = None;
                                output.send(WhatsAppEvent::HistorySyncCompleted).await;
                            }
                            Event::JoinedGroup(lazy_conv) => {
                                if let Some(conv) = lazy_conv.get() {
                                    let id = conv.id.clone();
                                    if !id.is_empty() {
                                        let jid = Jid::new(id.clone());
                                        let name = conv
                                            .display_name
                                            .clone()
                                            .or_else(|| conv.name.clone())
                                            .or_else(|| conv.username.clone())
                                            .or_else(|| conv.pn_jid.as_ref().map(|jid| Jid::new(jid.clone()).display_label()))
                                            .unwrap_or_else(|| Jid::new(id.clone()).display_label());

                                        let last_activity = conv
                                            .conversation_timestamp
                                            .or(conv.last_msg_timestamp)
                                            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0));

                                        let chat = Chat {
                                            jid,
                                            name,
                                            last_message: None,
                                            last_activity,
                                            is_group: id.contains("@g.us"),
                                            unread_count: conv.unread_count.unwrap_or(0),
                                            is_muted: conv.mute_end_time.unwrap_or(0) > 0,
                                            is_pinned: conv.pinned.unwrap_or(0) > 0,
                                        };

                                        output.send(WhatsAppEvent::ChatUpdated(chat)).await;

                                        if let Some(full_conv) = lazy_conv.get_with_messages() {
                                            for item in full_conv.messages {
                                                let Some(web) = item.message else { continue; };
                                                let Some(message) = web.message else { continue; };

                                                let chat_jid = web
                                                    .key
                                                    .remote_jid
                                                    .clone()
                                                    .unwrap_or_else(|| id.clone());
                                                let sender_jid = web
                                                    .key
                                                    .participant
                                                    .clone()
                                                    .or_else(|| web.key.remote_jid.clone())
                                                    .unwrap_or_else(|| chat_jid.clone());

                                                let content = parse_message_content(&message);

                                                let timestamp = web
                                                    .message_timestamp
                                                    .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0))
                                                    .unwrap_or_else(Utc::now);

                                                let history_msg = ChatMessage {
                                                    id: web.key.id.unwrap_or_else(|| {
                                                        format!("history_{}_{}", chat_jid, timestamp.timestamp_millis())
                                                    }),
                                                    sender: Jid::new(sender_jid),
                                                    chat: Jid::new(chat_jid),
                                                    content,
                                                    timestamp,
                                                    is_from_me: web.key.from_me.unwrap_or(false),
                                                    status: MessageStatus::Delivered,
                                                    quoted_message: None,
                                                };

                                                output.send(WhatsAppEvent::MessageReceived(history_msg)).await;
                                            }
                                        }

                                        sync_current = sync_current.saturating_add(1);
                                        let total = sync_total_hint.unwrap_or(sync_current).max(sync_current);
                                        output
                                            .send(WhatsAppEvent::HistorySyncProgress {
                                                current: sync_current,
                                                total,
                                            })
                                            .await;
                                    }
                                }
                            }
                            Event::ContactUpdate(update) => {
                                let name = update
                                    .action
                                    .full_name
                                    .clone()
                                    .or(update.action.first_name.clone());

                                if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
                                    output
                                        .send(WhatsAppEvent::ContactNameUpdated {
                                            jid: Jid::new(update.jid.to_string()),
                                            name: name.clone(),
                                        })
                                        .await;

                                    if let Some(pn_jid) = update.action.pn_jid.as_ref() {
                                        output
                                            .send(WhatsAppEvent::ContactNameUpdated {
                                                jid: Jid::new(pn_jid.clone()),
                                                name: name.clone(),
                                            })
                                            .await;
                                    }

                                    if let Some(lid_jid) = update.action.lid_jid.as_ref() {
                                        output
                                            .send(WhatsAppEvent::ContactNameUpdated {
                                                jid: Jid::new(lid_jid.clone()),
                                                name,
                                            })
                                            .await;
                                    }
                                }
                            }
                            Event::PushNameUpdate(update) => {
                                if !update.new_push_name.trim().is_empty() {
                                    output
                                        .send(WhatsAppEvent::ContactNameUpdated {
                                            jid: Jid::new(update.jid.to_string()),
                                            name: update.new_push_name,
                                        })
                                        .await;
                                }
                            }
                            _ => {
                                // Handle other events as needed
                            }
                        }
                    }

                    // Handle commands from UI
                    Some(command) = command_rx.next() => {
                        match command {
                            WhatsAppCommand::SendMessage { local_id, chat_jid, text } => {
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
                                    match client.send_message(wa_jid, message).await {
                                        Ok(message_id) => {
                                            output
                                                .send(WhatsAppEvent::MessageSent {
                                                    local_id,
                                                    message_id,
                                                    chat_jid,
                                                })
                                                .await;
                                        }
                                        Err(e) => {
                                            output
                                                .send(WhatsAppEvent::MessageSendFailed {
                                                    local_id,
                                                    chat_jid,
                                                    error: e.to_string(),
                                                })
                                                .await;
                                        }
                                    }
                                } else {
                                    output
                                        .send(WhatsAppEvent::MessageSendFailed {
                                            local_id,
                                            chat_jid,
                                            error: "Invalid chat JID".to_string(),
                                        })
                                        .await;
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
                            WhatsAppCommand::FetchHistory {
                                chat_jid,
                                oldest_msg_id,
                                oldest_msg_from_me,
                                oldest_msg_timestamp_ms,
                                count,
                            } => {
                                use wacore_binary::jid::Jid as WaJid;

                                let jid_str = &chat_jid.0;
                                if let Some((user, server)) = jid_str.split_once('@') {
                                    let wa_jid = WaJid::new(user, server);
                                    if let Err(error) = client
                                        .fetch_message_history(
                                            &wa_jid,
                                            &oldest_msg_id,
                                            oldest_msg_from_me,
                                            oldest_msg_timestamp_ms,
                                            count,
                                        )
                                        .await
                                    {
                                        log::warn!(
                                            "Failed to fetch history for {}: {}",
                                            chat_jid,
                                            error
                                        );
                                    }
                                }
                            }
                            WhatsAppCommand::Disconnect => {
                                break;
                            }
                        }
                    }

                    // Bot task ended
                    _ = &mut bot_task => {
                        log::warn!("Bot task ended unexpectedly");
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

fn parse_message_content(message: &waproto::whatsapp::Message) -> MessageContent {
    if let Some(text) = message.conversation.as_ref() {
        return MessageContent::Text(text.clone());
    }

    if let Some(extended) = message.extended_text_message.as_ref() {
        if let Some(text) = extended.text.as_ref().filter(|s| !s.trim().is_empty()) {
            return MessageContent::Text(text.clone());
        }
        if let Some(description) = extended.description.as_ref().filter(|s| !s.trim().is_empty()) {
            return MessageContent::Text(description.clone());
        }
    }

    if let Some(image) = message.image_message.as_ref() {
        return MessageContent::Image {
            caption: image.caption.clone(),
            url: image.url.clone(),
            thumbnail: image.jpeg_thumbnail.clone(),
        };
    }

    if let Some(video) = message.video_message.as_ref() {
        return MessageContent::Video {
            caption: video.caption.clone(),
            url: video.url.clone(),
            thumbnail: video.jpeg_thumbnail.clone(),
        };
    }

    if let Some(audio) = message.audio_message.as_ref() {
        return MessageContent::Audio {
            url: audio.url.clone(),
            duration_secs: audio.seconds.unwrap_or(0),
            is_voice_note: audio.ptt.unwrap_or(false),
        };
    }

    if let Some(document) = message.document_message.as_ref() {
        return MessageContent::Document {
            filename: document.file_name.clone().unwrap_or_else(|| "Document".to_string()),
            url: document.url.clone(),
            mime_type: document.mimetype.clone(),
        };
    }

    if let Some(sticker) = message.sticker_message.as_ref() {
        return MessageContent::Sticker {
            url: sticker.url.clone(),
        };
    }

    if let Some(location) = message.location_message.as_ref() {
        return MessageContent::Location {
            latitude: location.degrees_latitude.unwrap_or(0.0),
            longitude: location.degrees_longitude.unwrap_or(0.0),
            name: location.name.clone(),
        };
    }

    if let Some(contact) = message.contact_message.as_ref() {
        return MessageContent::Contact {
            display_name: contact.display_name.clone().unwrap_or_else(|| "Contact".to_string()),
            vcard: contact.vcard.clone().unwrap_or_default(),
        };
    }

    if let Some(view_once) = message
        .view_once_message
        .as_ref()
        .and_then(|wrapper| wrapper.message.as_ref())
    {
        return parse_message_content(view_once);
    }

    if let Some(view_once_v2) = message
        .view_once_message_v2
        .as_ref()
        .and_then(|wrapper| wrapper.message.as_ref())
    {
        return parse_message_content(view_once_v2);
    }

    if let Some(ephemeral) = message
        .ephemeral_message
        .as_ref()
        .and_then(|wrapper| wrapper.message.as_ref())
    {
        return parse_message_content(ephemeral);
    }

    if let Some(edited) = message
        .edited_message
        .as_ref()
        .and_then(|wrapper| wrapper.message.as_ref())
    {
        return parse_message_content(edited);
    }

    MessageContent::Unknown
}
