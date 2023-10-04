use std::collections::HashMap;

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};

use crate::errors::{MuuzikaError, MuuzikaResult};
use crate::state::State;
use crate::ws;
use crate::ws::WsConnection;

#[derive(Serialize, Deserialize, Display, Debug, Clone, Eq, PartialEq, Hash, FromStr)]
pub struct RoomCode(String);

impl RoomCode {
    pub fn new(code: String) -> Self {
        Self(code)
    }
}

pub struct Room {
    pub state: State,
    pub code: RoomCode,
    pub players: HashMap<Username, Player>,
    pub leader: Username,
}

impl Room {
    pub fn new(state: State, code: RoomCode, leader: Player) -> Self {
        let mut players = HashMap::new();
        let leader_username = leader.username.clone();
        players.insert(leader_username.clone(), leader);
        Self {
            state,
            code,
            players,
            leader: leader_username,
        }
    }

    pub fn get_player_mut(&mut self, username: &Username) -> MuuzikaResult<&mut Player> {
        self.players
            .get_mut(username)
            .ok_or_else(|| MuuzikaError::PlayerNotInRoom {
                room_code: self.code.clone(),
                username: username.clone(),
            })
    }

    pub fn get_player(&self, username: &Username) -> MuuzikaResult<&Player> {
        self.players
            .get(username)
            .ok_or_else(|| MuuzikaError::PlayerNotInRoom {
                room_code: self.code.clone(),
                username: username.clone(),
            })
    }

    fn send_base<T>(&self, message: T, except: Option<&Username>) -> MuuzikaResult<()>
    where
        T: Serialize,
    {
        let message = ws::make_message(message, None)?;

        self.players
            .values()
            .filter_map(|player| {
                if let Some(except) = except {
                    if &player.username == except {
                        return None;
                    }
                }
                player.ws.as_ref()
            })
            .for_each(|ws| {
                ws.send_raw(message.clone());
            });

        Ok(())
    }

    pub fn send<T>(&self, message: T) -> MuuzikaResult<()>
    where
        T: Serialize,
    {
        self.send_base(message, None)
    }

    pub fn send_except<T>(&self, message: T, except: &Username) -> MuuzikaResult<()>
    where
        T: Serialize,
    {
        self.send_base(message, Some(except))
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoomDto {
    pub code: RoomCode,
    pub leader: Username,
    pub players: Vec<PlayerDto>,
}

impl From<&Room> for RoomDto {
    fn from(room: &Room) -> Self {
        Self {
            code: room.code.clone(),
            leader: room.leader.clone(),
            players: room
                .players
                .values()
                .map(|player| player.into())
                .collect::<Vec<PlayerDto>>(),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoomSyncDto {
    pub you: Username,
    pub room: RoomDto,
}

#[derive(Serialize, Deserialize, Display, Debug, Clone, Eq, PartialEq, Hash, FromStr)]
pub struct Username(String);
pub type Score = u32;

pub struct Player {
    username: Username,
    score: Score,
    pub ws: Option<WsConnection>,
    pub created_at: u64,
}

impl Player {
    pub fn new(username: Username) -> Self {
        Self {
            username,
            ws: None,
            score: 0,
            created_at: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDto {
    pub username: Username,
    pub score: Score,
    pub is_online: bool,
}

impl From<&Player> for PlayerDto {
    fn from(player: &Player) -> Self {
        Self {
            username: player.username.clone(),
            score: player.score,
            is_online: player.ws.is_some(),
        }
    }
}
