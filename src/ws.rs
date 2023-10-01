use std::fmt;

use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use nanoid::nanoid;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::{Rejection, Reply};

use crate::errors::MuuzikaError;
use crate::messages::{handle_client_message, ClientMessage, ServerMessage};
use crate::rooms::{RoomCode, Username};
use crate::state::{State, WrappedRoom};

const WS_LOG_TARGET: &'static str = "muuzika::ws";

fn split_and_spawn_flusher(ws: WebSocket) -> (WsConnection, SplitStream<WebSocket>) {
    let (mut user_ws_tx, user_ws_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx.send(message).await.unwrap_or_else(|e| {
                log::debug!(target: WS_LOG_TARGET, "WebSocket send error: {:?}", e);
            })
        }
    });

    let conn = WsConnection { id: nanoid!(), tx };

    (conn, user_ws_rx)
}

pub async fn handle_ws(
    room_code: RoomCode,
    ws: warp::ws::Ws,
    auth_header: String,
    state: State,
) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| handle_ws_upgrade(socket, state, room_code, auth_header)))
}

pub async fn handle_ws_upgrade(
    ws: WebSocket,
    state: State,
    room_code: RoomCode,
    auth_header: String,
) {
    const LOG_TARGET: &'static str = "muuzika::handle_ws_upgrade";

    let (conn, mut rx) = split_and_spawn_flusher(ws);

    log::debug!(target: LOG_TARGET, "{:?} | {:?} | Upgrading", conn, room_code);

    let room = if let Some(r) = state.rooms.read().await.get(&room_code) {
        r.clone()
    } else {
        log::debug!(target: LOG_TARGET, "{:?} | Room not found", conn);
        serialize_and_send_and_close(
            &conn,
            ServerMessage::Error(
                MuuzikaError::RoomNotFound {
                    room_code: room_code.clone(),
                }
                .into(),
            ),
        );
        return;
    };

    let username = match room
        .write()
        .await
        .connect_player(&auth_header, conn.clone())
    {
        Ok(sync) => {
            let username = sync.you.clone();
            serialize_and_send(&conn, ServerMessage::Sync(sync), None);
            username
        }
        Err(e) => {
            log::debug!(target: LOG_TARGET, "{:?} | {:?} | Error connecting player: {:?}", conn, room_code, e);
            serialize_and_send_and_close(&conn, ServerMessage::Error(e.into()));
            return;
        }
    };

    while let Some(result) = rx.next().await {
        let message = match result {
            Ok(m) => m,
            Err(e) => {
                log::debug!(target: WS_LOG_TARGET, "{:?} | {:?} | {:?} | Message error: {:?}", conn, room_code, username, e);
                break;
            }
        };
        handle_message(&conn, message, &username, &room_code, &room).await;
    }

    let _ = room.write().await.disconnect_player(&username, &conn);
}

async fn handle_message(
    conn: &WsConnection,
    message: Message,
    username: &Username,
    room_code: &RoomCode,
    room: &WrappedRoom,
) {
    let message = if let Ok(m) = message.to_str() {
        m
    } else {
        return;
    };

    const LOG_TARGET: &'static str = "muuzika::handle_message";

    log::trace!(target: LOG_TARGET, "{:?} | {:?} | {:?} | Received message: {}", conn, room_code, username, message);

    let value = match serde_json::from_str::<Value>(message) {
        Ok(v) => v,
        Err(e) => {
            log::debug!(target: LOG_TARGET, "{:?} | {:?} | {:?} | Error parsing message: {:?}", conn, room_code, username, e);
            serialize_and_send(
                conn,
                ServerMessage::Error(MuuzikaError::from(e).into()),
                None,
            );
            return;
        }
    };

    let ack = value
        .get("ack")
        .map(Value::as_str)
        .flatten()
        .map(String::from);

    let client_message = match serde_json::from_value::<ClientMessage>(value) {
        Ok(m) => m,
        Err(e) => {
            log::debug!(target: LOG_TARGET, "{:?} | {:?} | {:?} | Error parsing message: {:?}", conn, room_code, username, e);
            serialize_and_send(
                conn,
                ServerMessage::Error(MuuzikaError::from(e).into()),
                ack,
            );
            return;
        }
    };

    log::trace!(target: LOG_TARGET, "{:?} | {:?} | {:?} | Handling message: {:?}", conn, room_code, username, client_message);
    let result = handle_client_message(client_message, username, room).await;
    log::trace!(target: LOG_TARGET, "{:?} | {:?} | {:?} | Answering with: {:?}, ack={:?}", conn, room_code, username, result, ack);

    serialize_and_send(conn, result, ack);
}

pub fn make_message<T>(message: T, ack: Option<String>) -> serde_json::Result<Message>
where
    T: serde::Serialize,
{
    let value = serde_json::to_value(message)?;

    let text = match value {
        Value::Object(mut map) => {
            if let Some(ack) = ack {
                map.insert("ack".to_string(), Value::String(ack));
            }
            serde_json::to_string(&map)?
        }
        _ => serde_json::to_string(&value)?,
    };

    Ok(Message::text(text))
}

#[derive(Clone)]
pub struct WsConnection {
    pub id: String,
    pub tx: UnboundedSender<Message>,
}

impl fmt::Debug for WsConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WsConnection").field(&self.id).finish()
    }
}

impl PartialEq for WsConnection {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

fn close(conn: &WsConnection) {
    let _ = conn.tx.send(Message::close());
}

pub fn send_or_close(conn: &WsConnection, message: Message) -> bool {
    if let Ok(_) = conn.tx.send(message) {
        true
    } else {
        close(conn);
        false
    }
}

pub fn serialize_and_send<T>(conn: &WsConnection, message: T, ack: Option<String>) -> bool
where
    T: serde::Serialize,
{
    if let Ok(message) = make_message(message, ack) {
        send_or_close(conn, message)
    } else {
        close(conn);
        false
    }
}

pub fn serialize_and_send_and_close<T>(conn: &WsConnection, message: T)
where
    T: serde::Serialize,
{
    if serialize_and_send(conn, message, None) {
        close(conn);
    }
}
