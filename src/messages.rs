use serde::{Deserialize, Serialize};

use crate::errors::{ErrorResponse, MuuzikaResult};
use crate::rooms::{RoomSyncDto, Username};
use crate::state::WrappedRoom;

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Sync(RoomSyncDto),
    PlayerJoined(Username),
    PlayerLeft(Username),
    PlayerConnected(Username),
    PlayerDisconnected(Username),
    Noop,
    Error(ErrorResponse),
    Result(u32),
    AddResult { result: u32, username: Username },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    Add(Vec<u32>),
}

pub async fn handle_client_message(
    message: ClientMessage,
    username: &Username,
    room: &WrappedRoom,
) -> ServerMessage {
    let result: MuuzikaResult<ServerMessage> = match message {
        ClientMessage::Add(numbers) => handle_add(numbers, username, room).await,
    };

    result
        .map_err(ErrorResponse::from)
        .unwrap_or_else(ServerMessage::Error)
}

pub async fn handle_add(
    numbers: Vec<u32>,
    username: &Username,
    room: &WrappedRoom,
) -> MuuzikaResult<ServerMessage> {
    let result = numbers.iter().sum();

    room.read().await.send(ServerMessage::AddResult {
        result,
        username: username.clone(),
    })?;

    Ok(ServerMessage::Noop)
}
