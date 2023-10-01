use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::{Rejection, Reply};

use crate::errors::MuuzikaError;
use crate::messages::{handle_client_message, ClientMessage, ServerMessage};
use crate::rooms::{Room, RoomCode, Username};
use crate::state::{State, WrappedRoom};

fn split_and_spawn_flusher(ws: WebSocket) -> (UnboundedSender<Message>, SplitStream<WebSocket>) {
    let (mut user_ws_tx, user_ws_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx.send(message).await.unwrap_or_else(|e| {
                eprintln!("websocket send error: {}", e);
            })
        }
    });

    (tx, user_ws_rx)
}

pub async fn handle_ws(
    ws: warp::ws::Ws,
    state: State,
    room_code: RoomCode,
    username: Username,
) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| handle_ws_upgrade(socket, state, room_code, username)))
}

pub async fn handle_ws_upgrade(
    ws: WebSocket,
    state: State,
    room_code: RoomCode,
    username: Username,
) {
    let (tx, mut rx) = split_and_spawn_flusher(ws);

    let room = if let Some(r) = state.rooms.read().await.get(&room_code) {
        r.clone()
    } else {
        serialize_and_send_and_close(
            &tx,
            ServerMessage::Error(
                MuuzikaError::RoomNotFound {
                    room_code: room_code.clone(),
                }
                .into(),
            ),
        );
        return;
    };

    let room_code = {
        let mut room = room.write().await;
        if let Err(err) = room.connect_player(&username, tx.clone()) {
            serialize_and_send_and_close(&tx, ServerMessage::Error(err.into()));
            return;
        };
        serialize_and_send(&tx, ServerMessage::RoomSync((&room as &Room).into()), None);
        room.code.clone()
    };

    println!("[Room {}] User {} connected", room_code, username);

    while let Some(result) = rx.next().await {
        let message = match result {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "websocket error(room={}, player={}): {}",
                    room_code, username, e
                );
                break;
            }
        };
        handle_message(&tx, message, &username, room.clone()).await;
    }

    let _ = room.write().await.disconnect_player(&username);
    println!("[Room {}] User {} disconnected", room_code, username);
}

async fn handle_message(
    tx: &UnboundedSender<Message>,
    message: Message,
    username: &Username,
    room: WrappedRoom,
) {
    let message = if let Ok(m) = message.to_str() {
        m
    } else {
        return;
    };

    let value = match serde_json::from_str::<Value>(message) {
        Ok(v) => v,
        Err(e) => {
            serialize_and_send(tx, ServerMessage::Error(MuuzikaError::from(e).into()), None);
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
            serialize_and_send(tx, ServerMessage::Error(MuuzikaError::from(e).into()), ack);
            return;
        }
    };

    let result = handle_client_message(client_message, username, room).await;

    serialize_and_send(tx, result, ack);
}

fn close(tx: &UnboundedSender<Message>) {
    let _ = tx.send(Message::close());
}

pub fn make_message<T>(message: T, ack: Option<String>) -> serde_json::Result<Message>
where
    T: serde::Serialize,
{
    let value = serde_json::to_value(message)?;

    let text = match value {
        serde_json::Value::Object(mut map) => {
            if let Some(ack) = ack {
                map.insert("ack".to_string(), serde_json::Value::String(ack));
            }
            serde_json::to_string(&map)?
        }
        _ => serde_json::to_string(&value)?,
    };

    Ok(Message::text(text))
}

pub fn send_or_close(tx: &UnboundedSender<Message>, message: Message) -> bool {
    if let Ok(_) = tx.send(message) {
        true
    } else {
        close(tx);
        false
    }
}

pub fn serialize_and_send<T>(tx: &UnboundedSender<Message>, message: T, ack: Option<String>) -> bool
where
    T: serde::Serialize,
{
    if let Ok(message) = make_message(message, ack) {
        send_or_close(tx, message)
    } else {
        close(tx);
        false
    }
}

fn serialize_and_send_and_close<T>(tx: &UnboundedSender<Message>, message: T)
where
    T: serde::Serialize,
{
    if serialize_and_send(tx, message, None) {
        let _ = tx.send(Message::close());
    }
}
