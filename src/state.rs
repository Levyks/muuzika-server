use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::rooms::{generate_available_codes, RoomCode, WrappedRoom};

#[derive(Clone)]
pub struct State {
    pub rooms: Arc<RwLock<HashMap<RoomCode, WrappedRoom>>>,
    pub available_codes: Arc<RwLock<Vec<RoomCode>>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            available_codes: Arc::new(RwLock::new(generate_available_codes(10000))),
        }
    }
}