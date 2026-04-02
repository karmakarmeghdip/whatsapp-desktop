#[derive(Debug, Clone)]
pub struct Chat {
    pub id: usize,
    pub name: String,
    pub last_message: String,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub is_me: bool,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectChat(usize),
    InputChanged(String),
    SendMessage,
}
