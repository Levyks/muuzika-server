extern crate derive_more;

use std::env;

use warp::Filter;

use crate::filters::{filters, handle_rejection};
use crate::state::State;

mod auth;
mod errors;
mod filters;
#[macro_use]
mod helpers;
mod lobby;
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
    pretty_env_logger::init_timed();

    let state = State::new();
    let server = filters(state)
        .recover(handle_rejection)
        .with(warp::log("muuzika::http"));

    warp::serve(server).run(([0, 0, 0, 0], 3030)).await;
}
