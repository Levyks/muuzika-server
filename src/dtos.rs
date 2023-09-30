use crate::serialization::{serialize_status_code, serialize_utc_date_time};
use serde::{Deserialize, Serialize};
use warp::http::StatusCode;

use crate::rooms::RoomCode;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    #[serde(serialize_with = "serialize_status_code")]
    pub code: StatusCode,
    #[serde(serialize_with = "serialize_utc_date_time")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub error: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(
        code: StatusCode,
        error: String,
        message: String,
        data: Option<serde_json::Value>,
    ) -> Self {
        ErrorResponse {
            code,
            timestamp: chrono::Utc::now(),
            error,
            message,
            data,
        }
    }

    pub fn no_data(code: StatusCode, error: String, message: String) -> Self {
        ErrorResponse::new(code, error, message, None)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrJoinRoomRequest {
    pub username: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomJoinedResponse {
    pub room_code: RoomCode,
    pub username: String,
    pub token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "data")]
pub enum ServerToClientMessage {
    Ready,
    PlayerJoined { username: String },
    PlayerLeft { username: String },
    PlayerConnected { username: String },
    PlayerDisconnected { username: String },
    Error(ErrorResponse),
}
