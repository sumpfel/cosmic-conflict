//! Protocol shared by the game client and the server.
//! Messages are sent as newline-delimited JSON over TCP.

use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 7777;

pub type PlayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    Join { name: String },
    Move { position: [f32; 3] },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome { id: PlayerId },
    PlayerJoined { id: PlayerId, name: String },
    PlayerMoved { id: PlayerId, position: [f32; 3] },
    PlayerLeft { id: PlayerId },
}

pub fn encode<T: Serialize>(msg: &T) -> serde_json::Result<String> {
    let mut s = serde_json::to_string(msg)?;
    s.push('\n');
    Ok(s)
}

pub fn decode<'a, T: Deserialize<'a>>(line: &'a str) -> serde_json::Result<T> {
    serde_json::from_str(line.trim())
}
