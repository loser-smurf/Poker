mod card;
mod deck;
mod game;
mod hand;
mod player;
mod models;
mod commands;
mod network;
mod utils;

use crate::network::start_server;

#[tokio::main]
async fn main() {
    start_server().await;
}