use std::collections::HashMap;
use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::thread_rng;
use tokio::sync::RwLock;

use crate::rooms::{Room, RoomCode};

#[derive(Clone)]
pub struct State {
    pub rooms: Arc<RwLock<HashMap<RoomCode, WrappedRoom>>>,
    pub available_codes: Arc<RwLock<Vec<RoomCode>>>,
}

pub type WrappedRoom = Arc<RwLock<Room>>;

impl State {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            available_codes: Arc::new(RwLock::new(generate_available_codes(10000))),
        }
    }
}

fn generate_available_codes(max: u32) -> Vec<RoomCode> {
    let mut codes: Vec<u32> = (0..max).collect();
    codes.shuffle(&mut thread_rng());
    codes
}
