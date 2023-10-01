use std::sync::Arc;

use rand::random;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use warp::http::StatusCode;
use warp::{Rejection, Reply};

use crate::auth::encode_token;
use crate::errors::MuuzikaError;
use crate::messages::ServerMessage;
use crate::rooms::{Player, Room, RoomCode, Username};
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
    pub username: Username,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomJoinedResponse {
    pub room_code: RoomCode,
    pub token: String,
}

pub async fn create_room(
    request: CreateOrJoinRoomRequest,
    state: State,
) -> Result<impl Reply, Rejection> {
    let identifier = format!("{:05}", random::<u16>());
    const LOG_TARGET: &'static str = "muuzika::create_room";

    log::debug!(target: LOG_TARGET, "{} | Creating room for player {:?}", identifier, request.username);

    let room_code = {
        let mut available_codes = state.available_codes.write().await;
        if let Some(room_code) = available_codes.pop() {
            log::debug!(target: LOG_TARGET, "{} | Got room code {}, {} remaining", identifier, room_code, available_codes.len());
            room_code
        } else {
            log::error!(target: LOG_TARGET, "{} | Out of room codes", identifier);
            Err(MuuzikaError::OutOfRoomCodes)?
        }
    };

    let leader = Player::new(request.username.clone());
    let token = encode_token(
        &state.jwt_secret,
        leader.created_at,
        &room_code,
        &request.username,
    )?;
    let room = Room::new(state.clone(), room_code.clone(), leader);

    let wrapped_room = Arc::new(RwLock::new(room));

    {
        let mut rooms = state.rooms.write().await;
        rooms.insert(room_code.clone(), wrapped_room);
        log::debug!(target: LOG_TARGET, "{} | Room {} created, {} rooms total", identifier, room_code, rooms.len());
    }

    reply(RoomJoinedResponse { room_code, token }, StatusCode::CREATED)
}

pub async fn join_room(
    room_code: RoomCode,
    request: CreateOrJoinRoomRequest,
    state: State,
) -> Result<impl Reply, Rejection> {
    let identifier = format!("{:05}", random::<u16>());
    const LOG_TARGET: &'static str = "muuzika::join_room";

    log::debug!(target: LOG_TARGET, "{} | Joining room {:?} for player {:?}", identifier, room_code, request.username);

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
        log::debug!(target: LOG_TARGET, "{} | Username is already taken", identifier);
        Err(MuuzikaError::UsernameTaken {
            room_code: room_code.clone(),
            username: request.username.clone(),
        })?;
    }

    let player = Player::new(request.username.clone());
    let token = encode_token(
        &state.jwt_secret,
        player.created_at,
        &room_code,
        &request.username,
    )?;
    room.players.insert(request.username.clone(), player);

    log::debug!(target: LOG_TARGET, "{} | Player {:?} joined room {:?}", identifier, request.username, room_code);
    room.send(ServerMessage::PlayerJoined(request.username.clone()))?;

    reply(RoomJoinedResponse { room_code, token }, StatusCode::CREATED)
}
