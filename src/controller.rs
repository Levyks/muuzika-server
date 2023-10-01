use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use warp::http::StatusCode;
use warp::{Rejection, Reply};

use crate::errors::MuuzikaError;
use crate::messages::ServerMessage;
use crate::rooms::{Player, Room, RoomCode};
use crate::state::State;

fn reply<T>(data: T, status: StatusCode) -> Result<impl Reply, Rejection>
where
    T: serde::Serialize,
{
    Ok(warp::reply::with_status(warp::reply::json(&data), status))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrJoinRoomRequest {
    pub username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomJoinedResponse {
    pub room_code: RoomCode,
    pub username: String,
    pub token: String,
}

pub async fn create_room(
    request: CreateOrJoinRoomRequest,
    state: State,
) -> Result<impl Reply, Rejection> {
    let room_code = state
        .available_codes
        .write()
        .await
        .pop()
        .ok_or(MuuzikaError::OutOfRoomCodes)?;

    println!(
        "Creating room with code {} for player {}",
        room_code, request.username
    );

    let room = Room::new(state.clone(), room_code.clone(), request.username.clone());

    let wrapped_room = Arc::new(RwLock::new(room));

    state
        .rooms
        .write()
        .await
        .insert(room_code.clone(), wrapped_room);

    reply(
        RoomJoinedResponse {
            username: request.username,
            room_code,
            token: "token".to_string(),
        },
        StatusCode::CREATED,
    )
}

pub async fn join_room(
    room_code: RoomCode,
    request: CreateOrJoinRoomRequest,
    state: State,
) -> Result<impl Reply, Rejection> {
    let wrapped_room = state
        .rooms
        .read()
        .await
        .get(&room_code)
        .ok_or_else(|| MuuzikaError::RoomNotFound {
            room_code: room_code.clone(),
        })?
        .clone();

    let mut room = wrapped_room.write().await;

    if room.players.contains_key(&request.username) {
        Err(MuuzikaError::UsernameTaken {
            room_code: room_code.clone(),
            username: request.username.clone(),
        })?;
    }

    let player = Player::new(request.username.clone());

    room.players.insert(request.username.clone(), player);

    room.send(ServerMessage::PlayerJoined(request.username.clone()))?;

    reply(
        RoomJoinedResponse {
            username: request.username,
            room_code,
            token: "token".to_string(),
        },
        StatusCode::CREATED,
    )
}
