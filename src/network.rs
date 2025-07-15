use crate::models::*;
use crate::commands::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
use std::sync::{Arc, Mutex};

/// Starts the TCP server and listens for incoming connections.
pub async fn start_server() {
    let state = Arc::new(Mutex::new(ServerState::default()));
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("Server is running on port 8080");

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            handle_client(socket, state).await;
        });
    }
}

/// Handles a single client connection: reads commands, processes them, and sends responses.
pub async fn handle_client(socket: TcpStream, state: Arc<Mutex<ServerState>>) {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut user_id: Option<UserId> = None;

    // Create a channel for sending messages to this client
    let (tx, mut rx): (UnboundedSender<String>, UnboundedReceiver<String>) = unbounded_channel();

    // Task for sending messages from the channel to the writer
    tokio::spawn(async move {
        let mut writer = writer;
        while let Some(msg) = rx.recv().await {
            let _ = writer.write_all(msg.as_bytes()).await;
        }
    });

    // After creating the channel and the task for the writer:
    let _ = tx.send("Welcome to Poker Server!\n".to_string());
    let _ = tx.send("Commands: REGISTER <name>, CREATE_TABLE <table>, JOIN_TABLE <table>, LIST_TABLES, SHOW, QUIT\n".to_string());

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await.unwrap();
        if bytes == 0 {
            break;
        }
        let cmd = line.trim();
        let mut parts = cmd.split_whitespace();
        // Register the sender for the user after REGISTER
        match parts.clone().next() {
            Some("REGISTER") => {
                if let Some(name) = parts.clone().nth(1) {
                    let mut state = state.lock().unwrap();
                    state.writers.insert(name.to_string(), tx.clone());
                }
            },
            _ => {}
        }
        // Show cards to the player if they have an active game and hole_cards
        match parts.clone().next() {
            Some("REGISTER") | Some("CREATE_TABLE") | Some("LIST_TABLES") | Some("QUIT") => {},
            _ => {
                let (mut cards, mut pot, mut comm_cards) = (None, 0.0, vec![]);
                'outer: {
                    let state = state.lock().unwrap();
                    if let Some(ref uid) = user_id {
                        for table in state.tables.values() {
                            if let Some(game) = &table.game {
                                for player in &game.players {
                                    if player.name == *uid && !player.hole_cards.is_empty() {
                                        cards = Some(player.hole_cards.clone());
                                        pot = game.get_pot();
                                        comm_cards = game.get_community_cards().to_vec();
                                        break 'outer;
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(cards) = cards {
                    let _ = tx.send(format!(
                        "Your cards: {:?}\nPot: {}\nCommunity cards: {:?}\n",
                        cards, pot, comm_cards
                    ).to_string());
                }
            }
        }
        match parts.next() {
            Some("REGISTER") => {
                if let Some(name) = parts.next() {
                    handle_register(name, &state, &tx, &mut user_id);
                } else {
                    let _ = tx.send("Usage: REGISTER <name>\n".to_string());
                }
            }
            Some("CREATE_TABLE") => {
                if let Some(table) = parts.next() {
                    handle_create_table(table, &state, &tx);
                } else {
                    let _ = tx.send("Usage: CREATE_TABLE <table>\n".to_string());
                }
            }
            Some("JOIN_TABLE") => {
                if let Some(table) = parts.next() {
                    handle_join_table(&user_id, table, &state, &tx);
                } else {
                    let _ = tx.send("Usage: JOIN_TABLE <table>\n".to_string());
                }
            }
            Some("LIST_TABLES") => {
                handle_list_tables(&state, &tx);
            }
            Some("SHOW") => {
                handle_show(&user_id, &state, &tx).await;
            }
            Some("QUIT") => {
                handle_quit(&tx).await;
                break;
            }
            Some("BET") => {
                if let Some(amount_str) = parts.next() {
                    let amount: f64 = amount_str.parse().unwrap_or(0.0);
                    handle_bet(&user_id, amount, &state, &tx).await;
                } else {
                    let _ = tx.send("Usage: BET <amount>\n".to_string());
                }
            }
            Some("CALL") => {
                handle_call(&user_id, &state, &tx).await;
            }
            Some("CHECK") => {
                handle_check(&user_id, &state, &tx).await;
            }
            Some("FOLD") => {
                handle_fold(&user_id, &state, &tx).await;
            }
            Some("SHOW_STATE") => {
                handle_show_state(&state, &user_id, &tx).await;
            }
            Some(cmd) => {
                let _ = tx.send(format!("Unknown command: {}\n", cmd).to_string());
            }
            None => {}
        }
    }
}
