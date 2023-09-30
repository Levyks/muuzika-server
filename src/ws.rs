use futures_util::{SinkExt, StreamExt};
use futures_util::stream::SplitStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::{Rejection, Reply};
use warp::ws::{Message, WebSocket};

use crate::rooms::{RoomCode, Username, WrappedRoom};
use crate::state::State;

fn split_and_spawn_flusher(ws: WebSocket) -> (UnboundedSender<Message>, SplitStream<WebSocket>) {
    let (mut user_ws_tx, user_ws_rx) = ws.split();
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx
                .send(message)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("websocket send error: {}", e);
                })
        }
    });

    (tx, user_ws_rx)
}

pub async fn handle_ws(ws: warp::ws::Ws, state: State, room_code: RoomCode, username: Username) -> Result<impl Reply + Sized, Rejection> {
    println!("request for room {} by {}", room_code, username);
    if let Some(room_entry) = state.rooms.get(&room_code) {
        let room = room_entry.clone();
        Ok(ws.on_upgrade(move |socket| handle_ws_upgrade(socket, username, room)))
    } else {
        Err(warp::reject::not_found())
    }
}

pub async fn handle_ws_upgrade(ws: WebSocket, username: Username, room: WrappedRoom) {
    let (mut tx, mut rx) = split_and_spawn_flusher(ws);

    let room_code: RoomCode;
    {
        let mut room = room.write().await;
        room_code = room.code.clone();
        if room.connect_player(&username, tx.clone()).is_err() {
            println!("couldn't connect player to room");
            let _ = tx.send(Message::close());
            return;
        };
    }

    println!("[Room {}] User {} connected", room_code, username);

    while let Some(result) = rx.next().await {
        let message = match result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("websocket error(room={}, player={}): {}", room_code, username, e);
                break;
            }
        };
        handle_message(message, &username, room.clone()).await;
    }

    let _ = room.write().await.disconnect_player(&username);
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

    room.read().await.players.values().filter_map(|p| p.tx.as_ref()).for_each(|tx| {
        let _ = tx.send(Message::text(new_msg.clone()));
    });
}