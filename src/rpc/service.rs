//! RPC Service - bridges RPC layer to actual WhatsApp service
//!
//! This module runs the actual WhatsApp service and translates
//! between RPC types and internal whatsapp types.

use std::sync::Arc;
use futures::channel::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use super::{RpcNotification, RpcRequest};
use crate::whatsapp::{self, WhatsAppEvent, WhatsAppCommand};
use crate::whatsapp::storage::{self, StoredMessage};
use wacore::types::events::Event;
use chrono::{DateTime, Utc};
use prost::Message as _;

/// Run the RPC service that handles requests and sends notifications
pub async fn run_rpc_service(
    mut request_rx: mpsc::Receiver<RpcRequest>,
    notification_tx: UnboundedSender<RpcNotification>,
) {
    use futures::StreamExt;

    // Channel for events from whatsapp service
    let (wa_event_tx, mut wa_event_rx) = tokio::sync::mpsc::unbounded_channel::<WhatsAppEvent>();
    
    // Channel for commands to whatsapp service
    let (wa_cmd_tx, wa_cmd_rx) = mpsc::channel::<WhatsAppCommand>(100);
    
    // Store connection handle for sending commands
    let mut whatsapp_connection: Option<whatsapp::Connection> = None;

    // Start the WhatsApp service in a separate task
    let wa_handle = tokio::spawn(run_whatsapp_connection(wa_event_tx, wa_cmd_rx));
    tokio::pin!(wa_handle);

    // Main loop handling requests and forwarding events
    loop {
        tokio::select! {
            Some(request) = request_rx.next() => {
                handle_request(request, &mut whatsapp_connection, &wa_cmd_tx).await;
            }

            Some(event) = wa_event_rx.recv() => {
                // Store connection when we get Connected event
                if let WhatsAppEvent::Connected(conn) = &event {
                    whatsapp_connection = Some(conn.clone());
                }
                
                if let Some(notification) = convert_event_to_notification(event) {
                    if notification_tx.send(notification).is_err() {
                        log::error!("Failed to send notification - channel closed");
                        break;
                    }
                }
            }

            _ = &mut wa_handle => {
                log::warn!("WhatsApp service task ended");
                let _ = notification_tx.send(RpcNotification::Error(
                    "WhatsApp service stopped unexpectedly".to_string()
                ));
                break;
            }
        }
    }
}

/// Run the WhatsApp connection loop
async fn run_whatsapp_connection(
    event_tx: tokio::sync::mpsc::UnboundedSender<WhatsAppEvent>,
    _command_rx: mpsc::Receiver<WhatsAppCommand>,
) {
    use whatsapp_rust::bot::Bot;
    use whatsapp_rust::TokioRuntime;
    use whatsapp_rust::store::SqliteStore;
    use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
    use whatsapp_rust_ureq_http_client::UreqHttpClient;
    use futures::StreamExt;

    loop {
        // Send connecting event
        let _ = event_tx.send(WhatsAppEvent::ConnectionStateChanged(
            whatsapp::ConnectionState::Connecting
        ));

        // Initialize storage backend
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("whatsapp-desktop")
            .join("whatsapp.db");

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                log::error!("Failed to create data directory: {}", e);
                let _ = event_tx.send(WhatsAppEvent::Error(format!("Failed to create data directory: {}", e)));
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        }

        log::info!("Using database: {}", db_path.display());

        // Spawn storage writer
        let storage_writer = storage::spawn_writer(db_path.clone());

        // Load stored chats and messages
        let (stored_chats, stored_messages) = storage::load_snapshot(&db_path);
        
        // Send stored chats to UI
        for chat in stored_chats {
            let _ = event_tx.send(WhatsAppEvent::ChatUpdated(chat));
        }
        
        // Send stored messages to UI
        for stored in stored_messages {
            if let Ok(raw) = waproto::whatsapp::Message::decode(stored.raw_message.as_slice()) {
                let content = parse_message_content(&raw);
                let timestamp = DateTime::<Utc>::from_timestamp_millis(stored.timestamp_ms)
                    .unwrap_or_else(Utc::now);

                let _ = event_tx.send(WhatsAppEvent::MessageReceived(whatsapp::ChatMessage {
                    id: stored.message_id,
                    sender: whatsapp::Jid::new(stored.sender_jid),
                    chat: whatsapp::Jid::new(stored.chat_jid),
                    content,
                    timestamp,
                    is_from_me: stored.is_from_me,
                    status: stored.status,
                    quoted_message: None,
                }));
            }
        }

        let backend = match SqliteStore::new(db_path.to_string_lossy().as_ref()).await {
            Ok(store) => Arc::new(store),
            Err(e) => {
                log::error!("Failed to initialize storage: {}", e);
                let _ = event_tx.send(WhatsAppEvent::Error(format!("Failed to initialize storage: {}", e)));
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        // Create channels
        let (internal_event_tx, mut internal_event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<WhatsAppCommand>(100);
        
        // Create the connection handle
        let connection = whatsapp::Connection(cmd_tx.clone());

        // Build the bot
        let event_tx_clone = internal_event_tx.clone();
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
                let _ = event_tx.send(WhatsAppEvent::Error(format!("Failed to build bot: {}", e)));
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        let client = bot.client();

        // Start the bot
        log::info!("Starting bot...");
        let bot_handle_result = bot.run().await;
        let bot_join_handle = match bot_handle_result {
            Ok(handle) => {
                log::info!("Bot started successfully");
                handle
            }
            Err(e) => {
                log::error!("Failed to start bot: {}", e);
                let _ = event_tx.send(WhatsAppEvent::Error(format!("Failed to start bot: {}", e)));
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                continue;
            }
        };

        // Run bot task
        let mut bot_task = tokio::spawn(async move {
            let _ = bot_join_handle.await;
        });

        // Track sync progress
        let mut sync_current: u32 = 0;
        let mut sync_total_hint: Option<u32> = None;

        // Process events and commands
        loop {
            tokio::select! {
                Some(event) = internal_event_rx.recv() => {
                    handle_internal_event(event, &connection, &event_tx, &storage_writer, &mut sync_current, &mut sync_total_hint).await;
                }
                
                Some(command) = cmd_rx.next() => {
                    // Handle commands inline since client is not Send in a separate function
                    use wacore_binary::jid::Jid as WaJid;
                    use whatsapp_rust::ChatStateType;
                    
                    match command {
                        WhatsAppCommand::SendMessage { local_id, chat_jid, text } => {
                            let jid_str = chat_jid.0.clone();
                            if let Some((user, server)) = jid_str.split_once('@') {
                                let wa_jid = WaJid::new(user, server);
                                let message = waproto::whatsapp::Message {
                                    conversation: Some(text),
                                    ..Default::default()
                                };
                                
                                match client.send_message(wa_jid, message).await {
                                    Ok(message_id) => {
                                        log::info!("Message sent: {} -> {}", local_id, message_id);
                                        let _ = event_tx.send(WhatsAppEvent::MessageSent {
                                            local_id,
                                            message_id,
                                            chat_jid,
                                        });
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to send message: {}", e);
                                        let _ = event_tx.send(WhatsAppEvent::MessageSendFailed {
                                            local_id,
                                            chat_jid,
                                            error: e.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                        WhatsAppCommand::SendTyping { chat_jid, typing } => {
                            let jid_str = chat_jid.0;
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
                        WhatsAppCommand::MarkAsRead { .. } => {
                            // TODO: Implement mark as read
                        }
                        WhatsAppCommand::FetchHistory { .. } => {
                            // TODO: Implement fetch history
                        }
                        WhatsAppCommand::Disconnect => {
                            // TODO: Implement disconnect
                        }
                    }
                }
                
                _ = &mut bot_task => {
                    log::warn!("Bot task ended");
                    break;
                }
            }
        }

        // Connection ended
        let _ = event_tx.send(WhatsAppEvent::Disconnected);
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
}

async fn handle_internal_event(
    event: Event,
    connection: &whatsapp::Connection,
    event_tx: &tokio::sync::mpsc::UnboundedSender<WhatsAppEvent>,
    storage_writer: &storage::StorageWriter,
    sync_current: &mut u32,
    sync_total_hint: &mut Option<u32>,
) {
    use wacore::types::presence::{
        ChatPresence as WaChatPresence,
        ChatPresenceMedia as WaChatPresenceMedia,
        ReceiptType,
    };

    match event {
        Event::PairingQrCode { code, .. } => {
            log::info!("QR code received for pairing");
            let _ = event_tx.send(WhatsAppEvent::QrCodeReceived { qr_code: code.clone() });
            let _ = event_tx.send(WhatsAppEvent::ConnectionStateChanged(
                whatsapp::ConnectionState::WaitingForQr { qr_code: code }
            ));
        }
        Event::PairingCode { code, .. } => {
            log::info!("Pair code received: {}", code);
            let _ = event_tx.send(WhatsAppEvent::PairCodeReceived { code: code.clone() });
            let _ = event_tx.send(WhatsAppEvent::ConnectionStateChanged(
                whatsapp::ConnectionState::WaitingForPairCode { code }
            ));
        }
        Event::Connected(_) => {
            log::info!("Connected to WhatsApp");
            let _ = event_tx.send(WhatsAppEvent::Connected(connection.clone()));
            let _ = event_tx.send(WhatsAppEvent::ConnectionStateChanged(
                whatsapp::ConnectionState::Connected
            ));
        }
        Event::Disconnected(_) => {
            log::warn!("Disconnected from WhatsApp");
            let _ = event_tx.send(WhatsAppEvent::ConnectionStateChanged(
                whatsapp::ConnectionState::Reconnecting
            ));
        }
        Event::LoggedOut(_) => {
            log::warn!("Logged out from WhatsApp");
            let _ = event_tx.send(WhatsAppEvent::ConnectionStateChanged(
                whatsapp::ConnectionState::LoggedOut
            ));
        }
        Event::Message(msg, info) => {
            let content = parse_message_content(msg.as_ref());
            
            let chat_msg = whatsapp::ChatMessage {
                id: info.id.clone(),
                sender: whatsapp::Jid::new(info.source.sender.to_string()),
                chat: whatsapp::Jid::new(info.source.chat.to_string()),
                content,
                timestamp: info.timestamp,
                is_from_me: info.source.is_from_me,
                status: whatsapp::MessageStatus::Delivered,
                quoted_message: None,
            };

            // Persist message to storage
            storage_writer.persist_message(StoredMessage {
                message_id: info.id.clone(),
                sender_jid: info.source.sender.to_string(),
                chat_jid: info.source.chat.to_string(),
                is_from_me: info.source.is_from_me,
                timestamp_ms: info.timestamp.timestamp_millis(),
                status: whatsapp::MessageStatus::Delivered,
                raw_message: msg.encode_to_vec(),
            });

            // Update contact name if provided
            if !info.push_name.is_empty() {
                storage_writer.persist_contact_name(
                    info.source.sender.to_string(),
                    info.push_name.clone(),
                );
                let _ = event_tx.send(WhatsAppEvent::ContactNameUpdated {
                    jid: whatsapp::Jid::new(info.source.sender.to_string()),
                    name: info.push_name,
                });
            }

            let _ = event_tx.send(WhatsAppEvent::MessageReceived(chat_msg));
        }
        Event::Receipt(receipt) => {
            let status = match receipt.r#type {
                ReceiptType::Read | ReceiptType::ReadSelf => whatsapp::MessageStatus::Read,
                ReceiptType::Delivered => whatsapp::MessageStatus::Delivered,
                _ => whatsapp::MessageStatus::Sent,
            };
            
            for msg_id in receipt.message_ids {
                let _ = event_tx.send(WhatsAppEvent::MessageStatusUpdated {
                    message_id: msg_id.clone(),
                    chat_jid: whatsapp::Jid::new(receipt.source.chat.to_string()),
                    status,
                });
            }
        }
        Event::ChatPresence(update) => {
            let state = match (update.state, update.media) {
                (WaChatPresence::Composing, WaChatPresenceMedia::Audio) => whatsapp::TypingState::Recording,
                (WaChatPresence::Composing, _) => whatsapp::TypingState::Typing,
                (WaChatPresence::Paused, _) => whatsapp::TypingState::Idle,
            };
            
            let _ = event_tx.send(WhatsAppEvent::TypingIndicator {
                chat_jid: whatsapp::Jid::new(update.source.chat.to_string()),
                sender_jid: whatsapp::Jid::new(update.source.sender.to_string()),
                state,
            });
        }
        Event::Presence(update) => {
            let _ = event_tx.send(WhatsAppEvent::PresenceUpdated(whatsapp::Presence {
                jid: whatsapp::Jid::new(update.from.to_string()),
                is_online: !update.unavailable,
                last_seen: update.last_seen,
            }));
        }
        Event::OfflineSyncPreview(preview) => {
            *sync_current = 0;
            *sync_total_hint = Some(preview.total.max(0) as u32);
            let _ = event_tx.send(WhatsAppEvent::HistorySyncProgress {
                current: 0,
                total: preview.total as u32,
            });
        }
        Event::OfflineSyncCompleted(_) => {
            *sync_total_hint = None;
            let _ = event_tx.send(WhatsAppEvent::HistorySyncCompleted);
        }
        Event::JoinedGroup(lazy_conv) => {
            if let Some(conv) = lazy_conv.get() {
                let id = conv.id.clone();
                if !id.is_empty() {
                    let jid = whatsapp::Jid::new(id.clone());
                    let name = conv
                        .display_name
                        .clone()
                        .or_else(|| conv.name.clone())
                        .or_else(|| conv.username.clone())
                        .or_else(|| conv.pn_jid.as_ref().map(|jid| whatsapp::Jid::new(jid.clone()).display_label()))
                        .unwrap_or_else(|| whatsapp::Jid::new(id.clone()).display_label());

                    let last_activity = conv
                        .conversation_timestamp
                        .or(conv.last_msg_timestamp)
                        .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0));

                    let chat = whatsapp::Chat {
                        jid: jid.clone(),
                        name,
                        last_message: None,
                        last_activity,
                        is_group: id.contains("@g.us"),
                        unread_count: conv.unread_count.unwrap_or(0),
                        is_muted: conv.mute_end_time.unwrap_or(0) > 0,
                        is_pinned: conv.pinned.unwrap_or(0) > 0,
                    };

                    let raw_conversation = conv.encode_to_vec();
                    storage_writer.persist_chat(chat.clone(), Some(raw_conversation));
                    let _ = event_tx.send(WhatsAppEvent::ChatUpdated(chat));

                    // Process messages if available
                    if let Some(full_conv) = lazy_conv.get_with_messages() {
                        let total_messages = full_conv.messages.len();
                        for (idx, item) in full_conv.messages.into_iter().enumerate() {
                            let Some(web) = item.message else { continue; };
                            let Some(message) = web.message else { continue; };

                            let chat_jid = web.key.remote_jid.clone().unwrap_or_else(|| id.clone());
                            let sender_jid = web.key.participant.clone()
                                .or_else(|| web.key.remote_jid.clone())
                                .unwrap_or_else(|| chat_jid.clone());

                            let content = parse_message_content(&message);
                            let timestamp = web.message_timestamp
                                .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0))
                                .unwrap_or_else(Utc::now);

                            let history_msg = whatsapp::ChatMessage {
                                id: web.key.id.clone().unwrap_or_else(|| {
                                    format!("history_{}_{}", chat_jid, timestamp.timestamp_millis())
                                }),
                                sender: whatsapp::Jid::new(sender_jid),
                                chat: whatsapp::Jid::new(chat_jid),
                                content,
                                timestamp,
                                is_from_me: web.key.from_me.unwrap_or(false),
                                status: whatsapp::MessageStatus::Delivered,
                                quoted_message: None,
                            };

                            let raw_message = message.encode_to_vec();
                            storage_writer.persist_message(StoredMessage {
                                message_id: history_msg.id.clone(),
                                sender_jid: history_msg.sender.0.clone(),
                                chat_jid: history_msg.chat.0.clone(),
                                is_from_me: history_msg.is_from_me,
                                timestamp_ms: history_msg.timestamp.timestamp_millis(),
                                status: history_msg.status,
                                raw_message,
                            });

                            // Only send recent messages to UI during bulk sync
                            if idx + 80 >= total_messages {
                                let _ = event_tx.send(WhatsAppEvent::MessageReceived(history_msg));
                            }
                        }
                    }

                    *sync_current = sync_current.saturating_add(1);
                    let total = sync_total_hint.unwrap_or(*sync_current).max(*sync_current);
                    let _ = event_tx.send(WhatsAppEvent::HistorySyncProgress {
                        current: *sync_current,
                        total,
                    });
                }
            }
        }
        Event::ContactUpdate(update) => {
            let name = update.action.full_name.clone()
                .or(update.action.first_name.clone());

            if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
                storage_writer.persist_contact_name(update.jid.to_string(), name.clone());
                let _ = event_tx.send(WhatsAppEvent::ContactNameUpdated {
                    jid: whatsapp::Jid::new(update.jid.to_string()),
                    name: name.clone(),
                });

                if let Some(pn_jid) = update.action.pn_jid.as_ref() {
                    storage_writer.persist_contact_name(pn_jid.clone(), name.clone());
                    let _ = event_tx.send(WhatsAppEvent::ContactNameUpdated {
                        jid: whatsapp::Jid::new(pn_jid.clone()),
                        name: name.clone(),
                    });
                }

                if let Some(lid_jid) = update.action.lid_jid.as_ref() {
                    storage_writer.persist_contact_name(lid_jid.clone(), name.clone());
                    let _ = event_tx.send(WhatsAppEvent::ContactNameUpdated {
                        jid: whatsapp::Jid::new(lid_jid.clone()),
                        name,
                    });
                }
            }
        }
        Event::PushNameUpdate(update) => {
            if !update.new_push_name.trim().is_empty() {
                storage_writer.persist_contact_name(
                    update.jid.to_string(),
                    update.new_push_name.clone(),
                );
                let _ = event_tx.send(WhatsAppEvent::ContactNameUpdated {
                    jid: whatsapp::Jid::new(update.jid.to_string()),
                    name: update.new_push_name,
                });
            }
        }
        _ => {
            // Handle other events as needed
        }
    }
}

fn parse_message_content(message: &waproto::whatsapp::Message) -> whatsapp::MessageContent {
    if let Some(text) = message.conversation.as_ref() {
        return whatsapp::MessageContent::Text(text.clone());
    }

    if let Some(extended) = message.extended_text_message.as_ref() {
        if let Some(text) = extended.text.as_ref().filter(|s| !s.trim().is_empty()) {
            return whatsapp::MessageContent::Text(text.clone());
        }
        if let Some(description) = extended.description.as_ref().filter(|s| !s.trim().is_empty()) {
            return whatsapp::MessageContent::Text(description.clone());
        }
    }

    if let Some(image) = message.image_message.as_ref() {
        return whatsapp::MessageContent::Image {
            caption: image.caption.clone(),
            url: image.url.clone(),
            thumbnail: image.jpeg_thumbnail.clone(),
        };
    }

    if let Some(video) = message.video_message.as_ref() {
        return whatsapp::MessageContent::Video {
            caption: video.caption.clone(),
            url: video.url.clone(),
            thumbnail: video.jpeg_thumbnail.clone(),
        };
    }

    if let Some(audio) = message.audio_message.as_ref() {
        return whatsapp::MessageContent::Audio {
            url: audio.url.clone(),
            duration_secs: audio.seconds.unwrap_or(0),
            is_voice_note: audio.ptt.unwrap_or(false),
        };
    }

    if let Some(document) = message.document_message.as_ref() {
        return whatsapp::MessageContent::Document {
            filename: document.file_name.clone().unwrap_or_else(|| "Document".to_string()),
            url: document.url.clone(),
            mime_type: document.mimetype.clone(),
        };
    }

    if let Some(sticker) = message.sticker_message.as_ref() {
        return whatsapp::MessageContent::Sticker {
            url: sticker.url.clone(),
        };
    }

    if let Some(location) = message.location_message.as_ref() {
        return whatsapp::MessageContent::Location {
            latitude: location.degrees_latitude.unwrap_or(0.0),
            longitude: location.degrees_longitude.unwrap_or(0.0),
            name: location.name.clone(),
        };
    }

    if let Some(contact) = message.contact_message.as_ref() {
        return whatsapp::MessageContent::Contact {
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

    whatsapp::MessageContent::Unknown
}

async fn handle_request(
    request: RpcRequest,
    connection: &mut Option<whatsapp::Connection>,
    _wa_cmd_tx: &mpsc::Sender<WhatsAppCommand>,
) {
    match request {
        RpcRequest::SendMessage {
            local_id,
            chat_jid,
            text,
        } => {
            log::info!("RPC: SendMessage to {}: {}", chat_jid, text);
            if let Some(conn) = connection {
                conn.send_message_with_id(local_id, whatsapp::Jid(chat_jid.0), text);
            }
        }
        RpcRequest::SendTyping { chat_jid, typing } => {
            log::info!("RPC: SendTyping {} {}", chat_jid, typing);
            if let Some(conn) = connection {
                conn.send_typing(whatsapp::Jid(chat_jid.0), typing);
            }
        }
        RpcRequest::MarkAsRead { chat_jid } => {
            log::info!("RPC: MarkAsRead {}", chat_jid);
            if let Some(conn) = connection {
                conn.mark_as_read(whatsapp::Jid(chat_jid.0));
            }
        }
        RpcRequest::FetchHistory {
            chat_jid,
            oldest_msg_id: _,
            oldest_msg_from_me: _,
            oldest_msg_timestamp_ms: _,
            count: _,
        } => {
            log::info!("RPC: FetchHistory {}", chat_jid);
        }
        RpcRequest::Disconnect => {
            log::info!("RPC: Disconnect");
        }
    }
}

fn convert_event_to_notification(event: WhatsAppEvent) -> Option<RpcNotification> {
    match event {
        // ServiceReady is emitted directly, not converted from WhatsAppEvent
        WhatsAppEvent::ConnectionStateChanged(state) => {
            Some(RpcNotification::ConnectionStateChanged(convert_connection_state(state)))
        }
        WhatsAppEvent::QrCodeReceived { qr_code } => {
            Some(RpcNotification::QrCodeReceived { qr_code })
        }
        WhatsAppEvent::PairCodeReceived { code } => {
            Some(RpcNotification::PairCodeReceived { code })
        }
        WhatsAppEvent::Connected(_) => Some(RpcNotification::ConnectionStateChanged(super::ConnectionState::Connected)),
        WhatsAppEvent::Disconnected => Some(RpcNotification::ConnectionStateChanged(super::ConnectionState::Reconnecting)),
        WhatsAppEvent::LoggedOut => Some(RpcNotification::ConnectionStateChanged(super::ConnectionState::LoggedOut)),
        WhatsAppEvent::MessageReceived(msg) => {
            Some(RpcNotification::MessageReceived(convert_chat_message(msg)))
        }
        WhatsAppEvent::MessageSent {
            local_id,
            message_id,
            chat_jid,
        } => Some(RpcNotification::MessageSent {
            local_id,
            message_id,
            chat_jid: convert_jid(chat_jid),
        }),
        WhatsAppEvent::MessageSendFailed {
            local_id,
            chat_jid,
            error,
        } => Some(RpcNotification::MessageSendFailed {
            local_id,
            chat_jid: convert_jid(chat_jid),
            error,
        }),
        WhatsAppEvent::MessageStatusUpdated {
            message_id,
            chat_jid,
            status,
        } => Some(RpcNotification::MessageStatusUpdated {
            message_id,
            chat_jid: convert_jid(chat_jid),
            status: convert_message_status(status),
        }),
        WhatsAppEvent::ChatsUpdated(chats) => {
            Some(RpcNotification::ChatsUpdated(chats.into_iter().map(convert_chat).collect()))
        }
        WhatsAppEvent::ChatUpdated(chat) => Some(RpcNotification::ChatUpdated(convert_chat(chat))),
        WhatsAppEvent::ContactNameUpdated { jid, name } => Some(RpcNotification::ContactNameUpdated {
            jid: convert_jid(jid),
            name,
        }),
        WhatsAppEvent::TypingIndicator {
            chat_jid,
            sender_jid,
            state,
        } => Some(RpcNotification::TypingIndicator {
            chat_jid: convert_jid(chat_jid),
            sender_jid: convert_jid(sender_jid),
            state: convert_typing_state(state),
        }),
        WhatsAppEvent::PresenceUpdated(presence) => {
            Some(RpcNotification::PresenceUpdated(convert_presence(presence)))
        }
        WhatsAppEvent::HistorySyncProgress { current, total } => {
            Some(RpcNotification::HistorySyncProgress { current, total })
        }
        WhatsAppEvent::HistorySyncCompleted => Some(RpcNotification::HistorySyncCompleted),
        WhatsAppEvent::Error(error) => Some(RpcNotification::Error(error)),
    }
}

fn convert_jid(jid: whatsapp::Jid) -> super::Jid {
    super::Jid(jid.0)
}

fn convert_connection_state(state: whatsapp::ConnectionState) -> super::ConnectionState {
    match state {
        whatsapp::ConnectionState::Disconnected => super::ConnectionState::Disconnected,
        whatsapp::ConnectionState::Connecting => super::ConnectionState::Connecting,
        whatsapp::ConnectionState::WaitingForQr { qr_code } => {
            super::ConnectionState::WaitingForQr { qr_code }
        }
        whatsapp::ConnectionState::WaitingForPairCode { code } => {
            super::ConnectionState::WaitingForPairCode { code }
        }
        whatsapp::ConnectionState::Connected => super::ConnectionState::Connected,
        whatsapp::ConnectionState::Reconnecting => super::ConnectionState::Reconnecting,
        whatsapp::ConnectionState::LoggedOut => super::ConnectionState::LoggedOut,
    }
}

fn convert_message_status(status: whatsapp::MessageStatus) -> super::MessageStatus {
    match status {
        whatsapp::MessageStatus::Pending => super::MessageStatus::Pending,
        whatsapp::MessageStatus::Sent => super::MessageStatus::Sent,
        whatsapp::MessageStatus::Delivered => super::MessageStatus::Delivered,
        whatsapp::MessageStatus::Read => super::MessageStatus::Read,
        whatsapp::MessageStatus::Failed => super::MessageStatus::Failed,
    }
}

fn convert_typing_state(state: whatsapp::TypingState) -> super::TypingState {
    match state {
        whatsapp::TypingState::Idle => super::TypingState::Idle,
        whatsapp::TypingState::Typing => super::TypingState::Typing,
        whatsapp::TypingState::Recording => super::TypingState::Recording,
    }
}

fn convert_chat_message(msg: whatsapp::ChatMessage) -> super::ChatMessage {
    super::ChatMessage {
        id: msg.id,
        sender: convert_jid(msg.sender),
        chat: convert_jid(msg.chat),
        content: convert_message_content(msg.content),
        timestamp: msg.timestamp,
        is_from_me: msg.is_from_me,
        status: convert_message_status(msg.status),
        quoted_message: msg.quoted_message.map(|q| Box::new(convert_chat_message(*q))),
    }
}

fn convert_message_content(content: whatsapp::MessageContent) -> super::MessageContent {
    use whatsapp::MessageContent as WaContent;

    match content {
        WaContent::Text(text) => super::MessageContent::Text(text),
        WaContent::Image {
            caption,
            url,
            thumbnail,
        } => super::MessageContent::Image {
            caption,
            url,
            thumbnail,
        },
        WaContent::Video {
            caption,
            url,
            thumbnail,
        } => super::MessageContent::Video {
            caption,
            url,
            thumbnail,
        },
        WaContent::Audio {
            url,
            duration_secs,
            is_voice_note,
        } => super::MessageContent::Audio {
            url,
            duration_secs,
            is_voice_note,
        },
        WaContent::Document {
            filename,
            url,
            mime_type,
        } => super::MessageContent::Document {
            filename,
            url,
            mime_type,
        },
        WaContent::Sticker { url } => super::MessageContent::Sticker { url },
        WaContent::Location {
            latitude,
            longitude,
            name,
        } => super::MessageContent::Location {
            latitude,
            longitude,
            name,
        },
        WaContent::Contact {
            display_name,
            vcard,
        } => super::MessageContent::Contact {
            display_name,
            vcard,
        },
        WaContent::System(text) => super::MessageContent::System(text),
        WaContent::Unknown => super::MessageContent::Unknown,
    }
}

fn convert_chat(chat: whatsapp::Chat) -> super::Chat {
    super::Chat {
        jid: convert_jid(chat.jid),
        name: chat.name,
        last_message: chat.last_message,
        last_activity: chat.last_activity,
        is_group: chat.is_group,
        unread_count: chat.unread_count,
        is_muted: chat.is_muted,
        is_pinned: chat.is_pinned,
    }
}

fn convert_presence(presence: whatsapp::Presence) -> super::Presence {
    super::Presence {
        jid: convert_jid(presence.jid),
        is_online: presence.is_online,
        last_seen: presence.last_seen,
    }
}