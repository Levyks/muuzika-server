use crate::dtos::ServerToClientMessage;
use crate::errors::MuuzikaError;
use crate::rooms;
use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::{Rejection, Reply};

use crate::rooms::{RoomCode, Username};
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
        send_and_close(
            tx,
            ServerToClientMessage::Error(
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
        if let Err(err) = rooms::connect_player(&mut room, &username, tx.clone()) {
            send_and_close(tx, ServerToClientMessage::Error(err.into()));
            return;
        };
        room.code.clone()
    };

    send(tx, ServerToClientMessage::Ready);

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
        handle_message(message, &username, room.clone()).await;
    }

    let mut room = room.write().await;
    let _ = rooms::disconnect_player(&mut room, &username);
    println!("[Room {}] User {} disconnected", room_code, username);
}

async fn handle_message(message: Message, username: &Username, room: WrappedRoom) {
    // Skip any non-Text messages...
    let message = if let Ok(s) = message.to_str() {
        s
    } else {
        return;
    };

    let new_msg = format!("[{}]: {}", username, message);

    room.read()
        .await
        .players
        .values()
        .filter_map(|p| p.tx.as_ref())
        .for_each(|tx| {
            let _ = tx.send(Message::text(new_msg.clone()));
        });
}

fn send(tx: UnboundedSender<Message>, message: ServerToClientMessage) {
    tx.send(message.into()).unwrap_or_else(|_| {
        let _ = tx.send(Message::close());
    })
}

fn send_and_close(tx: UnboundedSender<Message>, message: ServerToClientMessage) {
    let _ = tx.send(message.into());
    let _ = tx.send(Message::close());
}

impl From<ServerToClientMessage> for Message {
    fn from(message: ServerToClientMessage) -> Self {
        serde_json::to_string(&message)
            .map(Message::text)
            .unwrap_or_else(|_| Message::close())
    }
}
