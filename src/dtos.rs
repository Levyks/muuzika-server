use serde::{Deserialize, Serialize};

use crate::rooms::RoomCode;

#[derive(Deserialize)]
pub struct CreateOrJoinRoomRequest {
    pub username: String,
}

#[derive(Serialize)]
pub struct RoomJoinedResponse {
    pub room_code: RoomCode,
    pub username: String,
    pub token: String,
}

