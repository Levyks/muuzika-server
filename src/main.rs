use warp::Filter;

use crate::filters::{filters, handle_rejection};
use crate::state::State;

mod rooms;
mod ws;
mod filters;
mod controller;
mod state;
mod errors;
mod serialization;
mod dtos;

#[tokio::main]
async fn main() {
    let state = State::new();
    let server = filters(state).recover(handle_rejection);

    warp::serve(server)
        .run(([127, 0, 0, 1], 3030))
        .await;
}