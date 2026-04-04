//! RPC Client for UI layer
//!
//! Provides a handle for sending commands to the WhatsApp service
//! and receiving notifications via an Iced subscription.

use futures::channel::mpsc;
use iced::task::{Never, Sipper, sipper};
use super::{RpcNotification, RpcRequest};

/// Handle for sending commands to the WhatsApp service from UI
#[derive(Debug, Clone)]
pub struct RpcClientHandle {
    sender: mpsc::Sender<RpcRequest>,
}

impl RpcClientHandle {
    /// Send a request to the WhatsApp service
    pub fn send(&mut self, request: RpcRequest) {
        let _ = self.sender.try_send(request);
    }
}

/// RPC Client that creates the subscription for receiving notifications
pub struct RpcClient;

impl RpcClient {
    /// Create a new RPC client that connects to the WhatsApp service
    /// Returns a Sipper subscription for receiving notifications
    pub fn connect() -> impl Sipper<Never, RpcNotification> {
        sipper(|mut output| async move {
            let (request_tx, request_rx) = mpsc::channel::<RpcRequest>(100);
            let (notification_tx, mut notification_rx) =
                tokio::sync::mpsc::unbounded_channel::<RpcNotification>();

            // Store the handle globally so UI can send requests
            let handle = RpcClientHandle { sender: request_tx };
            super::set_rpc_client_handle(handle);

            // Notify UI that the service is ready
            let _ = output.send(RpcNotification::ServiceReady).await;

            // Start the RPC service in a background task
            let _service_handle = tokio::spawn(super::service::run_rpc_service(
                request_rx,
                notification_tx,
            ));

            // Forward notifications to the UI
            loop {
                tokio::select! {
                    Some(notification) = notification_rx.recv() => {
                        let _ = output.send(notification).await;
                    }
                    else => {
                        break;
                    }
                }
            }

            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        })
    }
}

/// Create an Iced subscription for WhatsApp RPC notifications
pub fn subscription() -> iced::Subscription<RpcNotification> {
    iced::Subscription::run(RpcClient::connect)
}
