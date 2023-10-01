use serde::{Deserialize, Serialize};

use crate::errors::{ErrorResponse, MuuzikaResult};
use crate::rooms::{RoomDto, Username};
use crate::state::WrappedRoom;

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    RoomSync(RoomDto),
    PlayerJoined(String),
    PlayerLeft(String),
    PlayerConnected(String),
    PlayerDisconnected(String),
    Error(ErrorResponse),
    Result(u32),
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Add(Vec<u32>),
}

pub async fn handle_client_message(
    message: ClientMessage,
    username: &Username,
    room: WrappedRoom,
) -> ServerMessage {
    let result: MuuzikaResult<ServerMessage> = match message {
        ClientMessage::Add(numbers) => handle_add(numbers).await,
    };

    result
        .map_err(ErrorResponse::from)
        .unwrap_or_else(ServerMessage::Error)
}

pub async fn handle_add(numbers: Vec<u32>) -> MuuzikaResult<ServerMessage> {
    let result = numbers.iter().sum();

    Ok(ServerMessage::Result(result))
}
