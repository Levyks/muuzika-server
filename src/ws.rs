use std::fmt;

use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use nanoid::nanoid;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::{Rejection, Reply};

use crate::errors::MuuzikaError;
use crate::lobby;
use crate::messages::{handle_client_message, ClientMessage, ServerMessage};
use crate::rooms::Username;
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

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

pub async fn handle_ws(
    ws: warp::ws::Ws,
    state: State,
    query: WsQuery,
) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| handle_ws_upgrade(socket, state, query.token)))
}

pub async fn handle_ws_upgrade(ws: WebSocket, state: State, token: String) {
    let (conn, mut rx) = split_and_spawn_flusher(ws);

    let (room, username) = match lobby::connect_player(&state, &token, &conn).await {
        Ok((room, sync)) => {
            let username = sync.you.clone();
            conn.send(ServerMessage::Sync(sync), None);
            (room, username)
        }
        Err(e) => {
            conn.send_and_close(ServerMessage::Error(e.into()));
            return;
        }
    };

    while let Some(result) = rx.next().await {
        let message = match result {
            Ok(m) => m,
            Err(e) => {
                log::debug!(target: WS_LOG_TARGET, "{:?} | {:?} | Message error: {:?}", conn, username, e);
                break;
            }
        };
        if let Ok(m) = message.to_str() {
            handle_text_message(&conn, &room, &username, m).await;
        }
    }

    let _ = lobby::disconnect_player(&state, &room, &username, &conn).await;
}

fn parse_message(message: &str) -> (serde_json::Result<ClientMessage>, Option<String>) {
    let value = match serde_json::from_str::<Value>(message) {
        Ok(v) => v,
        Err(e) => {
            return (Err(e), None);
        }
    };

    let ack = value
        .get("ack")
        .map(Value::as_str)
        .flatten()
        .map(String::from);

    (serde_json::from_value::<ClientMessage>(value), ack)
}

async fn handle_text_message(
    conn: &WsConnection,
    room: &WrappedRoom,
    username: &Username,
    message: &str,
) {
    const LOG_TARGET: &'static str = "muuzika::ws::handle_text_message";

    log::trace!(target: LOG_TARGET, "{:?} | {:?} | Received message: {}", conn, username, message);

    let (client_message, ack) = match parse_message(message) {
        (Ok(m), ack) => (m, ack),
        (Err(e), ack) => {
            log::debug!(target: LOG_TARGET, "{:?} | {:?} | Error parsing message: {:?}", conn, username, e);
            conn.send(ServerMessage::Error(MuuzikaError::from(e).into()), ack);
            return;
        }
    };

    log::trace!(target: LOG_TARGET, "{:?} | {:?} | Handling message: {:?}", conn, username, client_message);
    let result = handle_client_message(client_message, username, room).await;
    log::trace!(target: LOG_TARGET, "{:?} | {:?} | Answering with: {:?}, ack={:?}", conn, username, result, ack);

    conn.send(result, ack);
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

impl WsConnection {
    pub fn close(&self) {
        let _ = self.tx.send(Message::close());
    }

    pub fn send_raw(&self, message: Message) -> bool {
        self.tx.send(message).is_ok()
    }

    pub fn send<T>(&self, message: T, ack: Option<String>) -> bool
    where
        T: serde::Serialize,
    {
        if let Ok(message) = make_message(message, ack) {
            self.send_raw(message)
        } else {
            self.close();
            false
        }
    }

    pub fn send_and_close<T>(&self, message: T)
    where
        T: serde::Serialize,
    {
        if self.send(message, None) {
            self.close();
        }
    }
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
