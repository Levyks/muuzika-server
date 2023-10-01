use std::collections::HashMap;

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};

use crate::auth::decode_header;
use crate::errors::{MuuzikaError, MuuzikaResult};
use crate::messages::ServerMessage;
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
    const LOG_TARGET: &'static str = "muuzika::room";
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
            .ok_or_else(|| {
                log::debug!(target: Room::LOG_TARGET, "{:?} | Player {:?} not found", self.code, username);
                MuuzikaError::PlayerNotInRoom {
                    room_code: self.code.clone(),
                    username: username.clone(),
                }
            })
    }

    pub fn get_player(&self, username: &Username) -> MuuzikaResult<&Player> {
        self.players
            .get(username)
            .ok_or_else(|| {
                log::debug!(target: Room::LOG_TARGET, "{:?} | Player {:?} not found", self.code, username);
                MuuzikaError::PlayerNotInRoom {
                    room_code: self.code.clone(),
                    username: username.clone(),
                }
            })
    }

    pub fn connect_player(
        &mut self,
        auth_header: &String,
        ws: WsConnection,
    ) -> MuuzikaResult<RoomSyncDto> {
        let claims = decode_header(&self.state.jwt_secret, &auth_header)?;
        let room_code = self.code.clone();

        log::debug!(target: Room::LOG_TARGET, "{:?} | Decoded JWT claims: {:?}", room_code, claims);

        let player = self.get_player_mut(&claims.username)?;

        if claims.iat != player.created_at || claims.room_code != room_code {
            log::debug!(target: Room::LOG_TARGET, "{:?} | {:?} | Invalid JWT claims", room_code, claims.username);
            return Err(MuuzikaError::ExpiredToken);
        }

        let mut was_already_connected = false;
        if let Some(ws) = &player.ws {
            log::debug!(target: Room::LOG_TARGET, "{:?} | {:?} | Connected in another client", room_code, claims.username);
            ws::serialize_and_send_and_close(
                ws,
                ServerMessage::Error(MuuzikaError::ConnectedInAnotherDevice.into()),
            );
            was_already_connected = true;
        }

        player.ws = Some(ws);

        if !was_already_connected {
            log::debug!(target: Room::LOG_TARGET, "{:?} | {:?} | Player connected", room_code, claims.username);
            self.send_except(
                ServerMessage::PlayerConnected(claims.username.clone()),
                &claims.username,
            )?;
        }

        Ok(RoomSyncDto {
            you: claims.username.clone(),
            room: (self as &Room).into(),
        })
    }

    pub fn disconnect_player(
        &mut self,
        username: &Username,
        ws: &WsConnection,
    ) -> MuuzikaResult<()> {
        let player = self.get_player_mut(username)?;

        if let Some(old_ws) = &player.ws {
            if old_ws != ws {
                log::debug!(target: Room::LOG_TARGET, "{:?} | {:?} | Previous client disconnected", self.code, username);
                return Ok(());
            }
        }

        player.ws = None;

        log::debug!(target: Room::LOG_TARGET, "{:?} | {:?} | Player disconnected", self.code, username);
        self.send(ServerMessage::PlayerDisconnected(username.clone()))?;

        Ok(())
    }

    pub fn send<T>(&self, message: T) -> MuuzikaResult<()>
    where
        T: Serialize,
    {
        let message = ws::make_message(message, None)?;

        self.players
            .values()
            .filter_map(|player| player.ws.as_ref())
            .for_each(|ws| {
                ws::send_or_close(ws, message.clone());
            });

        Ok(())
    }

    pub fn send_except<T>(&self, message: T, except: &Username) -> MuuzikaResult<()>
    where
        T: Serialize,
    {
        let message = ws::make_message(message, None)?;

        self.players
            .values()
            .filter_map(|player| {
                if &player.username != except {
                    player.ws.as_ref()
                } else {
                    None
                }
            })
            .for_each(|ws| {
                ws::send_or_close(ws, message.clone());
            });

        Ok(())
    }
}

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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
