use std::convert::Infallible;

use serde::de::DeserializeOwned;
use warp::{Filter, Rejection, Reply};

use crate::controller;
use crate::dtos::CreateOrJoinRoomRequest;
use crate::errors::get_response_from_rejection;
use crate::rooms::{RoomCode, Username};
use crate::state::State;
use crate::ws::handle_ws;

fn ws(state: State) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone {
    warp::path!("ws")
        .and(warp::ws())
        .and(with_state(state))
        .and(warp::header::<RoomCode>("room-code"))
        .and(warp::header::<Username>("username"))
        .and_then(handle_ws)
}

fn create_room(state: State) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone {
    warp::path!("rooms")
        .and(warp::post())
        .and(json_body::<CreateOrJoinRoomRequest>())
        .and(with_state(state))
        .and_then(controller::create_room)
}

fn join_room(state: State) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone {
    warp::path!("rooms" / RoomCode)
        .and(warp::post())
        .and(json_body::<CreateOrJoinRoomRequest>())
        .and(with_state(state))
        .and_then(controller::join_room)
}

pub fn rooms(state: State) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone {
    create_room(state.clone())
        .or(join_room(state))
}


pub fn filters(state: State) -> impl Filter<Extract=impl Reply, Error=Rejection> + Clone {
    rooms(state.clone())
        .or(ws(state))
}

fn with_state(state: State) -> impl Filter<Extract=(State, ), Error=Infallible> + Clone {
    warp::any().map(move || state.clone())
}

fn json_body<T>() -> impl Filter<Extract=(T, ), Error=warp::Rejection> + Clone
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