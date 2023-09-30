use std::collections::HashMap;

use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

use crate::errors::{MuuzikaError, MuuzikaResult};
use crate::state::State;

pub type RoomCode = u32;
pub type Username = String;
pub type Score = u32;

pub struct Player {
    username: Username,
    pub tx: Option<UnboundedSender<Message>>,
    score: Score,
}

impl Player {
    pub fn new(username: Username) -> Self {
        Self {
            username,
            tx: None,
            score: 0,
        }
    }
}

pub struct Room {
    pub state: State,
    pub code: RoomCode,
    pub players: HashMap<Username, Player>,
    pub leader: Username,
}

impl Room {
    pub fn new(state: State, code: RoomCode, leader_username: Username) -> Self {
        let leader = Player::new(leader_username.clone());
        let mut players = HashMap::new();
        players.insert(leader_username.clone(), leader);
        Self {
            state,
            code,
            players,
            leader: leader_username,
        }
    }
}

pub fn connect_player(
    room: &mut Room,
    username: &Username,
    tx: UnboundedSender<Message>,
) -> MuuzikaResult<()> {
    let player = room
        .players
        .get_mut(username)
        .ok_or_else(|| MuuzikaError::PlayerNotInRoom {
            room_code: room.code,
            username: username.clone(),
        })?;
    player.tx = Some(tx);
    Ok(())
}

pub fn disconnect_player(room: &mut Room, username: &Username) -> MuuzikaResult<()> {
    let player = room
        .players
        .get_mut(username)
        .ok_or_else(|| MuuzikaError::PlayerNotInRoom {
            room_code: room.code,
            username: username.clone(),
        })?;
    player.tx = None;
    Ok(())
}
