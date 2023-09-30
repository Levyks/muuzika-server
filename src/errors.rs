use serde::Serialize;
use thiserror::Error;
use warp::http::StatusCode;
use warp::reject::Reject;
use warp::Rejection;

use crate::serialization::{serialize_status_code, serialize_utc_date_time};

#[derive(Error, Debug, Serialize)]
#[serde(tag = "error", content = "data")]
pub enum MuuzikaError {
    #[error("Unknown error")]
    Unknown,

    #[error("Room not found")]
    RoomNotFound,

    #[error("Out of room codes")]
    OutOfRoomCodes,

    #[error("Username taken")]
    UsernameTaken,
}

impl MuuzikaError {
    pub fn code(&self) -> StatusCode {
        match self {
            MuuzikaError::RoomNotFound => StatusCode::NOT_FOUND,
            MuuzikaError::OutOfRoomCodes => StatusCode::SERVICE_UNAVAILABLE,
            MuuzikaError::UsernameTaken => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl Reject for MuuzikaError {}

pub type MuuzikaResult<T> = Result<T, MuuzikaError>;

#[derive(Serialize)]
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
    fn new(code: StatusCode, error: String, message: String, data: Option<serde_json::Value>) -> Self {
        ErrorResponse {
            code,
            timestamp: chrono::Utc::now(),
            error,
            message,
            data,
        }
    }

    fn no_data(code: StatusCode, error: String, message: String) -> Self {
        ErrorResponse::new(code, error, message, None)
    }
}


impl From<&MuuzikaError> for ErrorResponse {
    fn from(muuzika_error: &MuuzikaError) -> Self {
        let error: String;
        let data: Option<serde_json::Value>;

        if let Some(json_value) = serde_json::to_value(muuzika_error).ok() {
            error = json_value
                .get("error")
                .map(|v| v.as_str())
                .flatten()
                .unwrap_or("Unknown")
                .to_string();

            data = json_value
                .get("data")
                .map(|v| v.clone());
        } else {
            error = "Unknown".to_string();
            data = None;
        }

        ErrorResponse::new(
            muuzika_error.code(),
            error,
            muuzika_error.to_string(),
            data,
        )
    }
}

// I'm sorry
pub fn get_response_from_rejection(err: Rejection) -> ErrorResponse {
    if let Some(muuzika_error) = err.find::<MuuzikaError>() {
        muuzika_error.into()
    } else if err.is_not_found() {
        ErrorResponse::no_data(
            StatusCode::NOT_FOUND,
            "NotFound".to_string(),
            "Not found".to_string(),
        )
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        ErrorResponse::no_data(
            StatusCode::METHOD_NOT_ALLOWED,
            "MethodNotAllowed".to_string(),
            "Method not allowed".to_string(),
        )
    } else if let Some(invalid_header) = err.find::<warp::reject::InvalidHeader>() {
        ErrorResponse::no_data(
            StatusCode::BAD_REQUEST,
            "InvalidHeader".to_string(),
            invalid_header.to_string(),
        )
    } else if let Some(missing_header) = err.find::<warp::reject::MissingHeader>() {
        ErrorResponse::no_data(
            StatusCode::BAD_REQUEST,
            "MissingHeader".to_string(),
            missing_header.to_string(),
        )
    } else if let Some(missing_cookie) = err.find::<warp::reject::MissingCookie>() {
        ErrorResponse::no_data(
            StatusCode::BAD_REQUEST,
            "MissingCookie".to_string(),
            missing_cookie.to_string(),
        )
    } else if let Some(invalid_query) = err.find::<warp::reject::InvalidQuery>() {
        ErrorResponse::no_data(
            StatusCode::BAD_REQUEST,
            "InvalidQuery".to_string(),
            invalid_query.to_string(),
        )
    } else if let Some(body_deserialize_error) = err.find::<warp::body::BodyDeserializeError>() {
        ErrorResponse::no_data(
            StatusCode::BAD_REQUEST,
            "BodyDeserializeError".to_string(),
            body_deserialize_error.to_string(),
        )
    } else if let Some(missing_connection_upgrade) = err.find::<warp::ws::MissingConnectionUpgrade>() {
        ErrorResponse::no_data(
            StatusCode::BAD_REQUEST,
            "MissingConnectionUpgrade".to_string(),
            missing_connection_upgrade.to_string(),
        )
    } else if let Some(length_required) = err.find::<warp::reject::LengthRequired>() {
        ErrorResponse::no_data(
            StatusCode::LENGTH_REQUIRED,
            "LengthRequired".to_string(),
            length_required.to_string(),
        )
    } else if let Some(payload_too_large) = err.find::<warp::reject::PayloadTooLarge>() {
        ErrorResponse::no_data(
            StatusCode::PAYLOAD_TOO_LARGE,
            "PayloadTooLarge".to_string(),
            payload_too_large.to_string(),
        )
    } else if let Some(unsupported_media_type) = err.find::<warp::reject::UnsupportedMediaType>() {
        ErrorResponse::no_data(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "UnsupportedMediaType".to_string(),
            unsupported_media_type.to_string(),
        )
    } else if let Some(cors_forbidden) = err.find::<warp::cors::CorsForbidden>() {
        ErrorResponse::no_data(
            StatusCode::FORBIDDEN,
            "CorsForbidden".to_string(),
            cors_forbidden.to_string(),
        )
    } else {
        ErrorResponse::no_data(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unknown".to_string(),
            "Unknown error".to_string(),
        )
    }
}