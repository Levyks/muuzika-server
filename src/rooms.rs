use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use rand::seq::SliceRandom;
use rand::thread_rng;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use warp::ws::Message;

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
    pub code: RoomCode,
    pub players: HashMap<Username, Player>,
}

impl Room {
    pub fn new(code: RoomCode) -> Self {
        Self {
            code,
            players: HashMap::new(),
        }
    }

    pub fn connect_player(&mut self, username: &Username, tx: UnboundedSender<Message>) -> Result<(), ()> {
        let player = self.players.get_mut(username).ok_or(())?;
        player.tx = Some(tx);
        Ok(())
    }

    pub fn disconnect_player(&mut self, username: &Username) -> Result<(), ()> {
        let player = self.players.get_mut(username).ok_or(())?;
        player.tx = None;
        Ok(())
    }
}

pub type WrappedRoom = Arc<RwLock<Room>>;
pub type WrappedRoomsMap = Arc<DashMap<RoomCode, WrappedRoom>>;

pub fn generate_available_codes(max: u32) -> Vec<RoomCode> {
    let mut codes: Vec<u32> = (0..max).collect();
    codes.shuffle(&mut thread_rng());
    codes
}