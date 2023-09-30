use serde::Serializer;
use warp::http::StatusCode;

pub fn serialize_status_code<S>(code: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    serializer.serialize_u16(code.as_u16())
}

pub fn serialize_utc_date_time<S>(date_time: &chrono::DateTime<chrono::Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    serializer.serialize_str(&date_time.to_rfc3339())
}