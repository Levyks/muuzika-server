use std::convert::Infallible;

use serde::de::DeserializeOwned;
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

use crate::errors::get_response_from_rejection;
use crate::lobby;
use crate::rooms::RoomCode;
use crate::state::State;
use crate::ws::{handle_ws, WsQuery};

fn ws(state: State) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("ws")
        .and(warp::ws())
        .and(with_state(state))
        .and(warp::query::<WsQuery>())
        .and_then(handle_ws)
}

fn create_room(state: State) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("rooms")
        .and(warp::post())
        .and(with_state(state))
        .and(json_body::<lobby::CreateOrJoinRoomRequest>())
        .and_then(|state, request| async move {
            lobby::create_room(&state, &request)
                .await
                .map_err(warp::reject::custom)
        })
        .map(|response| warp::reply::with_status(warp::reply::json(&response), StatusCode::CREATED))
}

fn join_room(state: State) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!("rooms" / RoomCode)
        .and(warp::post())
        .and(with_state(state))
        .and(json_body::<lobby::CreateOrJoinRoomRequest>())
        .and_then(|room_code, state, request| async move {
            lobby::join_room(&state, &room_code, &request)
                .await
                .map_err(warp::reject::custom)
        })
        .map(|response| warp::reply::with_status(warp::reply::json(&response), StatusCode::CREATED))
}

pub fn filters(state: State) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    ws(state.clone())
        .or(create_room(state.clone()))
        .or(join_room(state.clone()))
}

fn with_state(state: State) -> impl Filter<Extract = (State,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn json_body<T>() -> impl Filter<Extract = (T,), Error = Rejection> + Clone
where
    T: DeserializeOwned + Send,
{
    // When accepting a body, we want a JSON body
    // (and to reject huge payloads)...
    warp::body::content_length_limit(1024 * 16).and(warp::body::json::<T>())
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let response = get_response_from_rejection(err);

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        response.code,
    ))
}
