use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;

use crate::helpers::{get_env_or_default, get_env_or_panic};
use rand::thread_rng;
use tokio::sync::RwLock;

use crate::rooms::{Room, RoomCode};

#[derive(Clone)]
pub struct State {
    pub jwt_secret: String,
    pub rooms: Arc<RwLock<HashMap<RoomCode, WrappedRoom>>>,
    pub available_codes: Arc<RwLock<Vec<RoomCode>>>,
}

pub type WrappedRoom = Arc<RwLock<Room>>;

impl State {
    pub fn new() -> Self {
        let code_length = get_env_or_default("ROOM_CODE_LENGTH", 4);
        let available_codes = generate_available_codes(code_length);
        Self {
            jwt_secret: get_env_or_panic("JWT_SECRET"),
            rooms: Arc::new(RwLock::new(HashMap::new())),
            available_codes: Arc::new(RwLock::new(available_codes)),
        }
    }
}

fn generate_available_codes(code_length: u8) -> Vec<RoomCode> {
    if code_length > 9 {
        panic!("Room code cannot be longer than 9 characters");
    }

    let number_of_codes = 10u32.pow(code_length as u32);
    let mut codes: Vec<RoomCode> = (0..number_of_codes)
        .map(|c| RoomCode::new(format!("{:0width$}", c, width = code_length as usize)))
        .collect();
    codes.shuffle(&mut thread_rng());
    codes
}
