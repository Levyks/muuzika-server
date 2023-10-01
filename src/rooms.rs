use std::collections::HashMap;

use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

use crate::errors::{MuuzikaError, MuuzikaResult};
use crate::messages::ServerMessage;
use crate::state::State;
use crate::ws;

pub type RoomCode = u32;
pub struct Room {
    pub state: State,
    pub code: RoomCode,
    pub players: HashMap<Username, Player>,
    pub leader: Username,
}

impl Room {
    const LOG_TARGET: &'static str = "muuzika::room";
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

    pub fn get_player_mut(&mut self, username: &Username) -> MuuzikaResult<&mut Player> {
        self.players
            .get_mut(username)
            .ok_or_else(|| MuuzikaError::PlayerNotInRoom {
                room_code: self.code,
                username: username.clone(),
            })
    }

    pub fn connect_player(
        &mut self,
        username: &Username,
        tx: UnboundedSender<Message>,
    ) -> MuuzikaResult<()> {
        let player = self.get_player_mut(username)?;
        player.tx = Some(tx);

        log::debug!(target: Room::LOG_TARGET, "[{}] Player \"{}\" connected", self.code, username);
        self.send_except(ServerMessage::PlayerConnected(username.clone()), &username)?;

        Ok(())
    }

    pub fn disconnect_player(&mut self, username: &Username) -> MuuzikaResult<()> {
        let player = self.get_player_mut(username)?;
        player.tx = None;

        log::debug!(target: Room::LOG_TARGET, "[{}] Player \"{}\" disconnected", self.code, username);
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
            .filter_map(|player| player.tx.as_ref())
            .for_each(|tx| {
                ws::send_or_close(tx, message.clone());
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
                    player.tx.as_ref()
                } else {
                    None
                }
            })
            .for_each(|tx| {
                ws::send_or_close(tx, message.clone());
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
            code: room.code,
            leader: room.leader.clone(),
            players: room
                .players
                .values()
                .map(|player| player.into())
                .collect::<Vec<PlayerDto>>(),
        }
    }
}

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
            is_online: player.tx.is_some(),
        }
    }
}
