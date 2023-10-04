use std::sync::Arc;
use std::time::Duration;

use futures_util::TryFutureExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, RwLock};
use tokio::time::timeout;

use crate::auth::{decode_token, encode_token};
use crate::errors::{MuuzikaError, MuuzikaResult};
use crate::messages::ServerMessage;
use crate::rooms::{Player, Room, RoomCode, RoomSyncDto, Username};
use crate::state::{State, WrappedRoom};
use crate::ws::WsConnection;

#[derive(Deserialize, Debug)]
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
    state: &State,
    request: &CreateOrJoinRoomRequest,
) -> MuuzikaResult<RoomJoinedResponse> {
    const LOG_TARGET: &'static str = "muuzika::lobby::create_room";
    let identifier = log_identifier!();

    log::debug!(target: LOG_TARGET, "{} | Creating room, {:?}", identifier, request);

    let (room_code, remaining_codes) = pop_room_code(state).await.map_err(|e| {
        log::debug!(target: LOG_TARGET, "{} | Error obtaining room code: {:?}", identifier, e);
        e
    })?;

    log::debug!(target: LOG_TARGET, "{} | Got room code {}, {} remaining", identifier, room_code, remaining_codes);

    match create_room_with_code(state, &request.username, &room_code).await {
        Ok(response) => {
            log::debug!(target: LOG_TARGET, "{} | Created room {} with leader \"{}\" successfully", identifier, room_code, request.username);
            Ok(response)
        }
        Err(e) => {
            log::debug!(target: LOG_TARGET, "{} | Error creating room: {:?}, will return room code {}", identifier, e, room_code);
            let remaining_codes = push_room_code(state, room_code).await;
            log::debug!(target: LOG_TARGET, "{} | Returned room code, {} remaining", identifier, remaining_codes);
            Err(e)
        }
    }
}

pub async fn join_room(
    state: &State,
    room_code: &RoomCode,
    request: &CreateOrJoinRoomRequest,
) -> MuuzikaResult<RoomJoinedResponse> {
    const LOG_TARGET: &'static str = "muuzika::lobby::join_room";
    let identifier = log_identifier!();
    let error_logger = create_error_logger!(LOG_TARGET, identifier, "Error joining room");

    log::debug!(target: LOG_TARGET, "{} | Joining room {}, {:?}", identifier, room_code, request);

    let wrapped_room = state
        .rooms
        .read()
        .await
        .get(&room_code)
        .ok_or_else(|| MuuzikaError::RoomNotFound {
            room_code: room_code.clone(),
        })
        .map_err(error_logger)?
        .clone();

    let token = {
        let mut room = wrapped_room.write().await;

        if room.players.contains_key(&request.username) {
            Err(MuuzikaError::UsernameTaken {
                room_code: room_code.clone(),
                username: request.username.clone(),
            })
            .map_err(error_logger)?;
        }

        let player = Player::new(request.username.clone());
        let token = encode_token(
            &state.jwt_secret,
            player.created_at,
            &room_code,
            &request.username,
        )
        .map_err(error_logger)?;
        room.players.insert(request.username.clone(), player);

        log::debug!(target: LOG_TARGET, "{} | Player {} joined room {} successfully", identifier, request.username, room_code);
        room.send(ServerMessage::PlayerJoined(request.username.clone()))
            .map_err(error_logger)?;

        if let Some(tx) = room.cancel_cleanup.take() {
            log::debug!(target: LOG_TARGET, "{} | Cancelling cleanup for room {}", identifier, room_code);
            let _ = tx.send(());
        }

        token
    };

    schedule_player_cleanup(
        state.clone(),
        wrapped_room.clone(),
        request.username.clone(),
    )
    .await;

    Ok(RoomJoinedResponse {
        room_code: room_code.clone(),
        token,
    })
}

pub async fn connect_player(
    state: &State,
    token: &String,
    ws: &WsConnection,
) -> MuuzikaResult<(WrappedRoom, RoomSyncDto)> {
    const LOG_TARGET: &'static str = "muuzika::lobby::connect_player";
    let identifier = log_identifier!();
    let error_logger = create_error_logger!(LOG_TARGET, identifier, "Error connecting player");

    log::debug!(target: LOG_TARGET, "{} | Connecting player with token {}, {:?}", identifier, token, ws);

    let claims = decode_token(&state.jwt_secret, &token).map_err(error_logger)?;
    log::debug!(target: LOG_TARGET, "{} | Decoded token: {:?}", identifier, claims);

    let wrapped_room = state
        .rooms
        .read()
        .await
        .get(&claims.room_code)
        .ok_or_else(|| MuuzikaError::RoomNotFound {
            room_code: claims.room_code.clone(),
        })
        .map_err(error_logger)?
        .clone();

    let sync = {
        let mut room = wrapped_room.write().await;

        let player = room
            .get_player_mut(&claims.username)
            .map_err(error_logger)?;

        if claims.iat != player.created_at {
            Err(MuuzikaError::UsernameTaken {
                room_code: claims.room_code.clone(),
                username: claims.username.clone(),
            })
            .map_err(error_logger)?;
        }

        if let Some(old_ws) = &player.ws {
            log::debug!(target: LOG_TARGET, "{} | Player \"{}\" was connected in another client, closing old connection, old={:?}, new={:?}", identifier, claims.username, old_ws, ws);
            old_ws.send_and_close(ServerMessage::Error(
                MuuzikaError::ConnectedInAnotherDevice.into(),
            ));
        }

        player.ws = Some(ws.clone());
        let cancel_cleanup = player.cancel_cleanup.take();

        room.send_except(
            ServerMessage::PlayerConnected(claims.username.clone()),
            &claims.username,
        )
        .map_err(error_logger)?;

        log::debug!(target: LOG_TARGET, "{} | Player \"{}\" connected to room {} successfully", identifier, claims.username, room.code);

        if let Some(tx) = cancel_cleanup {
            log::debug!(target: LOG_TARGET, "{} | Cancelling cleanup for player \"{}\"", identifier, claims.username);
            let _ = tx.send(());
        }

        RoomSyncDto {
            you: claims.username.clone(),
            room: (&room as &Room).into(),
        }
    };

    Ok((wrapped_room, sync))
}

pub async fn disconnect_player(
    state: &State,
    wrapped_room: &WrappedRoom,
    username: &Username,
    ws: &WsConnection,
) -> MuuzikaResult<()> {
    const LOG_TARGET: &'static str = "muuzika::lobby::disconnect_player";
    let identifier = log_identifier!();
    let error_logger = create_error_logger!(LOG_TARGET, identifier, "Error disconnecting player");

    {
        let mut room = wrapped_room.write().await;
        let player = room.get_player_mut(username).map_err(error_logger)?;

        if let Some(old_ws) = &player.ws {
            if old_ws != ws {
                log::debug!(target: LOG_TARGET, "{} | Old connection of player \"{}\" was disconnected", identifier, username);
                return Ok(());
            }
        }

        player.ws = None;

        room.send(ServerMessage::PlayerDisconnected(username.clone()))
            .map_err(error_logger)?;
    }

    schedule_player_cleanup(state.clone(), wrapped_room.clone(), username.clone()).await;

    Ok(())
}

async fn create_room_with_code(
    state: &State,
    username: &Username,
    room_code: &RoomCode,
) -> MuuzikaResult<RoomJoinedResponse> {
    let leader = Player::new(username.clone());
    let token = encode_token(&state.jwt_secret, leader.created_at, &room_code, username)?;
    let room = Room::new(room_code.clone(), leader);

    let wrapped_room = Arc::new(RwLock::new(room));

    state
        .rooms
        .write()
        .await
        .insert(room_code.clone(), wrapped_room.clone());

    schedule_player_cleanup(state.clone(), wrapped_room, username.clone()).await;

    Ok(RoomJoinedResponse {
        room_code: room_code.clone(),
        token,
    })
}

async fn pop_room_code(state: &State) -> MuuzikaResult<(RoomCode, usize)> {
    let mut available_codes = state.available_codes.write().await;
    available_codes
        .pop()
        .map(|room_code| (room_code, available_codes.len()))
        .ok_or_else(|| MuuzikaError::OutOfRoomCodes)
}

async fn push_room_code(state: &State, room_code: RoomCode) -> usize {
    let mut available_codes = state.available_codes.write().await;
    available_codes.push(room_code);
    available_codes.len()
}

async fn schedule_player_cleanup(state: State, wrapped_room: WrappedRoom, username: Username) {
    const LOG_TARGET: &'static str = "muuzika::lobby::schedule_player_cleanup";

    let duration = Duration::from_secs(10);

    let rx = {
        let mut room = wrapped_room.write().await;
        let player = if let Ok(p) = room.get_player_mut(&username) {
            log::debug!(target: LOG_TARGET, "Scheduling cleanup for player \"{}\" in {} seconds", username, duration.as_secs());
            p
        } else {
            log::debug!(target: LOG_TARGET, "Attempted to schedule cleanup for player \"{}\" but player is not in room {}", username, room.code);
            return;
        };

        let (tx, rx) = oneshot::channel::<()>();
        player.cancel_cleanup = Some(tx);
        rx
    };

    tokio::spawn(async move {
        if let Err(_) = timeout(duration, rx).await {
            do_player_cleanup(state, wrapped_room, username).await;
        }
    });
}

async fn do_player_cleanup(state: State, wrapped_room: WrappedRoom, username: Username) {
    const LOG_TARGET: &'static str = "muuzika::lobby::do_player_cleanup";

    let is_empty = {
        let mut room = wrapped_room.write().await;

        let player = if let Ok(p) = room.get_player(&username) {
            p
        } else {
            return;
        };

        if player.ws.is_some() {
            log::debug!(target: LOG_TARGET, "Player {} is still connected, will not clean up", username);
            return;
        }

        log::debug!(target: LOG_TARGET, "Player {} is disconnected, cleaning up", username);
        room.players.remove(&username);

        let _ = room.send(ServerMessage::PlayerLeft(username.clone()));

        room.players.is_empty()
    };

    if is_empty {
        schedule_room_cleanup(state, wrapped_room.clone()).await;
    }
}

async fn schedule_room_cleanup(state: State, wrapped_room: WrappedRoom) {
    const LOG_TARGET: &'static str = "muuzika::lobby::schedule_room_cleanup";

    let duration = Duration::from_secs(10);

    log::debug!(target: LOG_TARGET, "Scheduling cleanup for room {} in {} seconds", wrapped_room.read().await.code, duration.as_secs());

    let (tx, rx) = oneshot::channel::<()>();
    wrapped_room.write().await.cancel_cleanup = Some(tx);
    tokio::spawn(async move {
        if let Err(_) = timeout(duration, rx).await {
            do_room_cleanup(state, wrapped_room).await;
        }
    });
}

async fn do_room_cleanup(state: State, wrapped_room: WrappedRoom) {
    const LOG_TARGET: &'static str = "muuzika::lobby::do_room_cleanup";
    let room = wrapped_room.read().await;

    if !room.players.is_empty() {
        log::debug!(target: LOG_TARGET, "Room {} is not empty, will not clean up", room.code);
        return;
    }

    log::debug!(target: LOG_TARGET, "Room {} is empty, cleaning up", room.code);
    state.rooms.write().await.remove(&room.code);
    push_room_code(&state, room.code.clone()).await;
}
