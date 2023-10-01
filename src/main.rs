use warp::Filter;

use crate::filters::{filters, handle_rejection};
use crate::state::State;

mod controller;
mod errors;
mod filters;
mod messages;
mod rooms;
mod serialization;
mod state;
mod ws;

#[tokio::main]
async fn main() {
    let state = State::new();
    let server = filters(state).recover(handle_rejection);

    warp::serve(server).run(([127, 0, 0, 1], 3030)).await;
}
