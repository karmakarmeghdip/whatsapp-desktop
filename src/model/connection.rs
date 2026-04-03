//! Connection state types for the application model

/// Current connection state to WhatsApp servers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Waiting for user to scan QR code
    WaitingForQr {
        /// The QR code data to display
        qr_code: String,
    },
    /// Waiting for user to enter pair code on phone
    WaitingForPairCode {
        /// The code to enter on phone
        code: String,
    },
    /// Connected and authenticated
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting,
    /// User logged out (need to re-pair)
    LoggedOut,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl ConnectionState {
    /// Check if currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Check if needs user action to pair
    pub fn needs_pairing(&self) -> bool {
        matches!(
            self,
            Self::WaitingForQr { .. } | Self::WaitingForPairCode { .. } | Self::LoggedOut
        )
    }
}

/// Which view should be displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewState {
    /// Loading/initializing the application
    #[default]
    Loading,
    /// Pairing screen (QR code or pair code)
    Pairing,
    /// Main chat list and conversation view
    Chats,
    /// Settings screen
    Settings,
}
