use std::sync::Arc;

use tokio::sync::RwLock;
use warp::{Rejection, Reply};
use warp::http::StatusCode;

use crate::dtos::{CreateOrJoinRoomRequest, RoomJoinedResponse};
use crate::errors::MuuzikaError;
use crate::rooms::{Player, Room, RoomCode};
use crate::state::State;

fn reply<T>(data: T, status: StatusCode) -> impl Reply
    where
        T: serde::Serialize,
{
    warp::reply::with_status(
        warp::reply::json(&data),
        status,
    )
}

pub async fn create_room(
    request: CreateOrJoinRoomRequest, state: State,
) -> Result<impl Reply, Rejection> {
    let room_code = state.available_codes.write().await.pop().ok_or(MuuzikaError::OutOfRoomCodes)?;

    println!("Creating room with code {} for player {}", room_code, request.username);

    let mut room = Room::new(room_code.clone());
    let player = Player::new(request.username.clone());
    room.players.insert(request.username.clone(), player);

    let wrapped_room = Arc::new(RwLock::new(room));

    state.rooms.insert(room_code.clone(), wrapped_room);

    Ok(reply(RoomJoinedResponse {
        username: request.username,
        room_code,
        token: "token".to_string(),
    }, StatusCode::CREATED))
}

pub async fn join_room(
    room_code: RoomCode, request: CreateOrJoinRoomRequest, state: State,
) -> Result<impl Reply, Rejection> {
    println!("Player {} joining room {}", request.username, room_code);

    let wrapped_room = state.rooms.get(&room_code).ok_or(MuuzikaError::RoomNotFound)?.clone();
    let mut room = wrapped_room.write().await;

    if room.players.contains_key(&request.username) {
        Err(MuuzikaError::UsernameTaken)?;
    }

    let player = Player::new(request.username.clone());

    room.players.insert(request.username.clone(), player);

    Ok(reply(RoomJoinedResponse {
        username: request.username,
        room_code,
        token: "token".to_string(),
    }, StatusCode::CREATED))
}
