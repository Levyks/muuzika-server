use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::rooms::{generate_available_codes, RoomCode, WrappedRoom};

#[derive(Clone)]
pub struct State {
    pub rooms: Arc<DashMap<RoomCode, WrappedRoom>>,
    pub available_codes: Arc<RwLock<Vec<RoomCode>>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(DashMap::new()),
            available_codes: Arc::new(RwLock::new(generate_available_codes(10000))),
        }
    }
}