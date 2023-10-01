use std::env;

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
    if env::var_os("RUST_LOG").is_none() {
        env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::init();

    let state = State::new();
    let server = filters(state)
        .recover(handle_rejection)
        .with(warp::log("muuzika::http"));

    warp::serve(server).run(([127, 0, 0, 1], 3030)).await;
}
