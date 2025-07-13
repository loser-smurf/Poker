use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use crate::game::Game;

pub type UserId = String;
pub type TableId = String;

#[derive(Debug)]
pub struct User {
    pub name: String,
    pub balance: f64,
    pub table: Option<TableId>,
}

#[derive(Debug)]
pub struct Table {
    pub id: TableId,
    pub players: HashSet<UserId>,
    pub game: Option<Game>,
}

#[derive(Debug, Default)]
pub struct ServerState {
    pub users: HashMap<UserId, User>,
    pub tables: HashMap<TableId, Table>,
}

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

async fn handle_client(socket: TcpStream, state: Arc<Mutex<ServerState>>) {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut user_id: Option<UserId> = None;

    writer.write_all(b"Welcome to Poker Server!\n").await.unwrap();
    writer.write_all(b"Commands: REGISTER <name>, CREATE_TABLE <table>, JOIN_TABLE <table>, LIST_TABLES, SHOW, QUIT\n").await.unwrap();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).await.unwrap();
        if bytes == 0 {
            break;
        }
        let cmd = line.trim();
        let mut parts = cmd.split_whitespace();
        match parts.next() {
            Some("REGISTER") => {
                if let Some(name) = parts.next() {
                    let mut already_exists = false;
                    {
                        let mut state = state.lock().unwrap();
                        if state.users.contains_key(name) {
                            already_exists = true;
                        } else {
                            state.users.insert(name.to_string(), User { name: name.to_string(), balance: 100.0, table: None });
                            user_id = Some(name.to_string());
                        }
                    }
                    if already_exists {
                        writer.write_all(b"Username already taken\n").await.unwrap();
                    } else {
                        writer.write_all(b"Registered successfully. Your balance: 100\n").await.unwrap();
                    }
                } else {
                    writer.write_all(b"Usage: REGISTER <name>\n").await.unwrap();
                }
            }
            Some("CREATE_TABLE") => {
                if let Some(table) = parts.next() {
                    let mut already_exists = false;
                    {
                        let mut state = state.lock().unwrap();
                        if state.tables.contains_key(table) {
                            already_exists = true;
                        } else {
                            state.tables.insert(table.to_string(), Table { id: table.to_string(), players: HashSet::new(), game: None });
                        }
                    }
                    if already_exists {
                        writer.write_all(b"Table already exists\n").await.unwrap();
                    } else {
                        writer.write_all(b"Table created\n").await.unwrap();
                    }
                } else {
                    writer.write_all(b"Usage: CREATE_TABLE <table>\n").await.unwrap();
                }
            }
            Some("JOIN_TABLE") => {
                if let (Some(ref uid), Some(table)) = (user_id.as_ref(), parts.next()) {
                    let table_key = table.to_string();
                    let user_key = uid.clone().to_string();
                    let mut joined = false;
                    let mut start_game = false;
                    let mut table_players = vec![];
                    let mut table_id = String::new();
                    {
                        let mut state = state.lock().unwrap();
                        if let Some(table_obj) = state.tables.get_mut(&table_key) {
                            table_obj.players.insert(user_key.clone());
                            joined = true;
                            if table_obj.players.len() == 2 {
                                let mut game = Game::new(5.0, 10.0);
                                for pname in &table_obj.players {
                                    game.add_player(pname.clone(), 100.0);
                                }
                                game.start_new_hand().ok();
                                table_obj.game = Some(game);
                                start_game = true;
                                table_players = table_obj.players.iter().cloned().collect();
                                table_id = table_obj.id.clone();
                            }
                        }
                        if let Some(user) = state.users.get_mut(&user_key) {
                            user.table = Some(table_key.clone());
                        }
                    } // MutexGuard отпущен
                    if joined {
                        writer.write_all(b"Joined table\n").await.unwrap();
                    } else {
                        writer.write_all(b"Table not found\n").await.unwrap();
                    }
                    if start_game {
                        for pname in table_players {
                            println!("[SERVER] Player {}: Game started at table {}", pname, table_id);
                        }
                    }
                } else {
                    writer.write_all(b"Usage: JOIN_TABLE <table>\n").await.unwrap();
                }
            }
            Some("LIST_TABLES") => {
                let list = {
                    let state = state.lock().unwrap();
                    state.tables.keys().cloned().collect::<Vec<_>>().join(", ")
                };
                writer.write_all(format!("Tables: {}\n", list).as_bytes()).await.unwrap();
            }
            Some("SHOW") => {
                let (user_name, user_balance, table_id_opt, table_info) = {
                    let state = state.lock().unwrap();
                    if let Some(ref uid) = user_id {
                        if let Some(user) = state.users.get(uid) {
                            let user_name = user.name.clone();
                            let user_balance = user.balance;
                            let table_id_opt = user.table.clone();
                            let mut table_info = String::new();
                            if let Some(ref table_id) = user.table {
                                if let Some(table) = state.tables.get(table_id) {
                                    table_info.push_str(&format!("Table: {}\nPlayers: {:?}\n", table.id, table.players));
                                    if let Some(ref game) = table.game {
                                        table_info.push_str(&format!("Pot: {}\nCommunity cards: {:?}\n", game.get_pot(), game.get_community_cards()));
                                        for (i, player) in game.players.iter().enumerate() {
                                            table_info.push_str(&format!("Player {}: {} | Cards: {:?} | Balance: {}\n", i, player.name, player.hole_cards, player.balance));
                                        }
                                    }
                                }
                            }
                            (user_name, user_balance, table_id_opt, table_info)
                        } else {
                            (String::new(), 0.0, None, String::new())
                        }
                    } else {
                        (String::new(), 0.0, None, String::new())
                    }
                };
                if !user_name.is_empty() {
                    writer.write_all(format!("You: {} | Balance: {}\n", user_name, user_balance).as_bytes()).await.unwrap();
                    if !table_info.is_empty() {
                        writer.write_all(table_info.as_bytes()).await.unwrap();
                    }
                }
            }
            Some("QUIT") => {
                writer.write_all(b"Bye!\n").await.unwrap();
                break;
            }
            Some(cmd) => {
                writer.write_all(format!("Unknown command: {}\n", cmd).as_bytes()).await.unwrap();
            }
            None => {}
        }
    }
}