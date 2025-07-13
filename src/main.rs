mod card;
mod deck;
mod player;
mod game;
mod hand;
mod server;

#[tokio::main]
async fn main() {
    server::start_server().await;
}