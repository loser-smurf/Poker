use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
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
    pub writers: HashMap<UserId, UnboundedSender<String>>,
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
                                    if &player.name == uid && !player.hole_cards.is_empty() {
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
                        let _ = tx.send("Username already taken\n".to_string());
                    } else {
                        let _ = tx.send("Registered successfully. Your balance: 100\n".to_string());
                    }
                } else {
                    let _ = tx.send("Usage: REGISTER <name>\n".to_string());
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
                        let _ = tx.send("Table already exists\n".to_string());
                    } else {
                        let _ = tx.send("Table created\n".to_string());
                    }
                } else {
                    let _ = tx.send("Usage: CREATE_TABLE <table>\n".to_string());
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
                            if table_obj.players.len() >= 2 {
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
                    } // MutexGuard released
                    if joined {
                        let _ = tx.send("Joined table\n".to_string());
                    } else {
                        let _ = tx.send("Table not found\n".to_string());
                    }
                    // After the game starts, send cards to all players
                    if start_game {
                        let (player_cards, pot, comm_cards) = {
                            let state = state.lock().unwrap();
                            let mut player_cards = HashMap::new();
                            let mut pot = 0.0;
                            let mut comm_cards = vec![];
                            if let Some(table) = state.tables.get(&table_id) {
                                if let Some(game) = &table.game {
                                    for player in &game.players {
                                        player_cards.insert(player.name.clone(), player.hole_cards.clone());
                                    }
                                    pot = game.get_pot();
                                    comm_cards = game.get_community_cards().to_vec();
                                }
                            }
                            (player_cards, pot, comm_cards)
                        };
                        // Distribute cards to all players
                        let state = state.lock().unwrap();
                        for pname in &table_players {
                            if let Some(sender) = state.writers.get(pname) {
                                if let Some(cards) = player_cards.get(pname) {
                                    let msg = format!("Your cards: {:?}\nPot: {}\nCommunity cards: {:?}\n", cards, pot, comm_cards);
                                    let _ = sender.send(msg);
                                }
                            }
                        }
                    }
                } else {
                    let _ = tx.send("Usage: JOIN_TABLE <table>\n".to_string());
                }
            }
            Some("LIST_TABLES") => {
                let list = {
                    let state = state.lock().unwrap();
                    state.tables.keys().cloned().collect::<Vec<_>>().join(", ")
                };
                let _ = tx.send(format!("Tables: {}\n", list).to_string());
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
                    let _ = tx.send(format!("You: {} | Balance: {}\n", user_name, user_balance).to_string());
                    if !table_info.is_empty() {
                        let _ = tx.send(table_info);
                    }
                }
            }
            Some("QUIT") => {
                let _ = tx.send("Bye!\n".to_string());
                break;
            }
            Some("BET") => {
                if let Some(amount_str) = parts.next() {
                    let amount: f64 = amount_str.parse().unwrap_or(0.0);
                    let mut result = String::new();
                    let mut next_player_name = None;
                    let mut round_ended = false;
                    let mut winner_info = None;
                    {
                        let mut state = state.lock().unwrap();
                        if let Some(ref uid) = user_id {
                            for table in state.tables.values_mut() {
                                if let Some(game) = &mut table.game {
                                    if let Some(idx) = game.players.iter().position(|p| &p.name == uid) {
                                        match game.player_action(idx, crate::player::PlayerAction::Raise(amount)) {
                                            Ok(_) => {
                                                result = format!("You bet {}\n", amount);
                                                if game.is_betting_round_complete() {
                                                    round_ended = true;
                                                    match game.current_round {
                                                        crate::game::BettingRound::PreFlop => game.deal_flop(),
                                                        crate::game::BettingRound::Flop => game.deal_turn(),
                                                        crate::game::BettingRound::Turn => game.deal_river(),
                                                        crate::game::BettingRound::River => {
                                                            // Showdown
                                                            let hands = crate::hand::evaluate_hand;
                                                            let mut best: Option<(String, crate::hand::EvaluatedHand)> = None;
                                                            let mut winner = None;
                                                            for player in &game.players {
                                                                let mut all_cards = player.hole_cards.clone();
                                                                all_cards.extend(game.community_cards.iter().cloned());
                                                                let hand = hands(&all_cards);
                                                                if best.is_none() || hand > best.as_ref().unwrap().1 {
                                                                    best = Some((player.name.clone(), hand.clone()));
                                                                    winner = Some(player.name.clone());
                                                                }
                                                            }
                                                            if let Some(winner) = winner {
                                                                winner_info = Some(winner);
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                } else {
                                                    game.next_player();
                                                }
                                                next_player_name = game.get_current_player().map(|p| p.name.clone());
                                            }
                                            Err(e) => result = format!("Bet error: {}\n", e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let _ = tx.send(result.clone());
                    // Send game state after action
                    send_game_state(&state, &user_id, &tx, next_player_name, round_ended, winner_info).await;
                } else {
                    let _ = tx.send("Usage: BET <amount>\n".to_string());
                }
            }
            Some("CALL") => {
                let mut result = String::new();
                let mut next_player_name = None;
                let mut round_ended = false;
                let mut winner_info = None;
                {
                    let mut state = state.lock().unwrap();
                    if let Some(ref uid) = user_id {
                        for table in state.tables.values_mut() {
                            if let Some(game) = &mut table.game {
                                if let Some(idx) = game.players.iter().position(|p| &p.name == uid) {
                                    match game.player_action(idx, crate::player::PlayerAction::Call) {
                                        Ok(_) => {
                                            result = "You called\n".to_string();
                                            if game.is_betting_round_complete() {
                                                round_ended = true;
                                                match game.current_round {
                                                    crate::game::BettingRound::PreFlop => game.deal_flop(),
                                                    crate::game::BettingRound::Flop => game.deal_turn(),
                                                    crate::game::BettingRound::Turn => game.deal_river(),
                                                    crate::game::BettingRound::River => {
                                                        // Showdown
                                                        let hands = crate::hand::evaluate_hand;
                                                        let mut best: Option<(String, crate::hand::EvaluatedHand)> = None;
                                                        let mut winner = None;
                                                        for player in &game.players {
                                                            let mut all_cards = player.hole_cards.clone();
                                                            all_cards.extend(game.community_cards.iter().cloned());
                                                            let hand = hands(&all_cards);
                                                            if best.is_none() || hand > best.as_ref().unwrap().1 {
                                                                best = Some((player.name.clone(), hand.clone()));
                                                                winner = Some(player.name.clone());
                                                            }
                                                        }
                                                        if let Some(winner) = winner {
                                                            winner_info = Some(winner);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            } else {
                                                game.next_player();
                                            }
                                            next_player_name = game.get_current_player().map(|p| p.name.clone());
                                        }
                                        Err(e) => result = format!("Call error: {}\n", e),
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = tx.send(result.clone());
                send_game_state(&state, &user_id, &tx, next_player_name, round_ended, winner_info).await;
            }
            Some("CHECK") => {
                let mut result = String::new();
                let mut next_player_name = None;
                let mut round_ended = false;
                let mut winner_info = None;
                {
                    let mut state = state.lock().unwrap();
                    if let Some(ref uid) = user_id {
                        for table in state.tables.values_mut() {
                            if let Some(game) = &mut table.game {
                                if let Some(idx) = game.players.iter().position(|p| &p.name == uid) {
                                    match game.player_action(idx, crate::player::PlayerAction::Check) {
                                        Ok(_) => {
                                            result = "You checked\n".to_string();
                                            if game.is_betting_round_complete() {
                                                round_ended = true;
                                                match game.current_round {
                                                    crate::game::BettingRound::PreFlop => game.deal_flop(),
                                                    crate::game::BettingRound::Flop => game.deal_turn(),
                                                    crate::game::BettingRound::Turn => game.deal_river(),
                                                    crate::game::BettingRound::River => {
                                                        // Showdown
                                                        let hands = crate::hand::evaluate_hand;
                                                        let mut best: Option<(String, crate::hand::EvaluatedHand)> = None;
                                                        let mut winner = None;
                                                        for player in &game.players {
                                                            let mut all_cards = player.hole_cards.clone();
                                                            all_cards.extend(game.community_cards.iter().cloned());
                                                            let hand = hands(&all_cards);
                                                            if best.is_none() || hand > best.as_ref().unwrap().1 {
                                                                best = Some((player.name.clone(), hand.clone()));
                                                                winner = Some(player.name.clone());
                                                            }
                                                        }
                                                        if let Some(winner) = winner {
                                                            winner_info = Some(winner);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            } else {
                                                game.next_player();
                                            }
                                            next_player_name = game.get_current_player().map(|p| p.name.clone());
                                        }
                                        Err(e) => result = format!("Check error: {}\n", e),
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = tx.send(result.clone());
                send_game_state(&state, &user_id, &tx, next_player_name, round_ended, winner_info).await;
            }
            Some("FOLD") => {
                let mut result = String::new();
                let mut next_player_name = None;
                let mut round_ended = false;
                let mut winner_info = None;
                {
                    let mut state = state.lock().unwrap();
                    if let Some(ref uid) = user_id {
                        for table in state.tables.values_mut() {
                            if let Some(game) = &mut table.game {
                                if let Some(idx) = game.players.iter().position(|p| &p.name == uid) {
                                    match game.player_action(idx, crate::player::PlayerAction::Fold) {
                                        Ok(_) => {
                                            result = "You folded\n".to_string();
                                            if game.is_betting_round_complete() {
                                                round_ended = true;
                                                match game.current_round {
                                                    crate::game::BettingRound::PreFlop => game.deal_flop(),
                                                    crate::game::BettingRound::Flop => game.deal_turn(),
                                                    crate::game::BettingRound::Turn => game.deal_river(),
                                                    crate::game::BettingRound::River => {
                                                        // Showdown
                                                        let hands = crate::hand::evaluate_hand;
                                                        let mut best: Option<(String, crate::hand::EvaluatedHand)> = None;
                                                        let mut winner = None;
                                                        for player in &game.players {
                                                            let mut all_cards = player.hole_cards.clone();
                                                            all_cards.extend(game.community_cards.iter().cloned());
                                                            let hand = hands(&all_cards);
                                                            if best.is_none() || hand > best.as_ref().unwrap().1 {
                                                                best = Some((player.name.clone(), hand.clone()));
                                                                winner = Some(player.name.clone());
                                                            }
                                                        }
                                                        if let Some(winner) = winner {
                                                            winner_info = Some(winner);
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            } else {
                                                game.next_player();
                                            }
                                            next_player_name = game.get_current_player().map(|p| p.name.clone());
                                        }
                                        Err(e) => result = format!("Fold error: {}\n", e),
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = tx.send(result.clone());
                send_game_state(&state, &user_id, &tx, next_player_name, round_ended, winner_info).await;
            }
            Some("SHOW_STATE") => {
                send_game_state(&state, &user_id, &tx, None, false, None).await;
            }
            Some(cmd) => {
                let _ = tx.send(format!("Unknown command: {}\n", cmd).to_string());
            }
            None => {}
        }
    }
}

async fn send_game_state(state: &Arc<Mutex<ServerState>>, user_id: &Option<UserId>, writer: &UnboundedSender<String>, _next_player: Option<String>, round_ended: bool, winner: Option<String>) {
    let (cards, pot, comm_cards, current_player, folded, winner_str) = {
        let state = state.lock().unwrap();
        let mut cards = None;
        let mut pot = 0.0;
        let mut comm_cards = vec![];
        let mut current_player = None;
        let mut folded = false;
        let winner_str: Option<String> = None;
        for table in state.tables.values() {
            if let Some(game) = &table.game {
                for player in &game.players {
                    if let Some(uid) = user_id {
                        if &player.name == uid {
                            cards = Some(player.hole_cards.clone());
                            folded = player.state == crate::player::PlayerState::Folded;
                        }
                    }
                }
                pot = game.get_pot();
                comm_cards = game.get_community_cards().to_vec();
                current_player = game.get_current_player().map(|p| p.name.clone());
            }
        }
        (cards, pot, comm_cards, current_player, folded, winner.clone())
    };
    if let Some(cards) = cards {
        let _ = writer.send(format!("Your cards: {:?}\nPot: {}\nCommunity cards: {:?}\n", cards, pot, comm_cards));
        let commands = "Available commands: BET <amount>, CALL, CHECK, FOLD, SHOW, SHOW_STATE, QUIT";
        let _ = writer.send(format!("{}\n", commands));
        if let Some(cp) = current_player {
            let is_your_turn = if let Some(uid) = user_id { &cp == uid } else { false };
            let _ = writer.send(format!("Current player: {}{}\n", cp, if is_your_turn { " (you)" } else { "" }));
            if is_your_turn {
                let _ = writer.send("Your turn! You have 15 seconds...\n".to_string());
                // Start a timer for 15 seconds for auto-fold
                let writer = writer.clone();
                let uid = cp.clone();
                let state = Arc::clone(state);
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                    let mut should_fold = false;
                    {
                        let mut state = state.lock().unwrap();
                        for table in state.tables.values_mut() {
                            if let Some(game) = &mut table.game {
                                if let Some(idx) = game.players.iter().position(|p| p.name == uid) {
                                    if let Some(current) = game.get_current_player() {
                                        if current.name == uid && game.players[idx].state != crate::player::PlayerState::Folded {
                                            should_fold = true;
                                            let _ = game.player_action(idx, crate::player::PlayerAction::Fold);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if should_fold {
                        let _ = writer.send("You did not act in time. Auto-folded.\n".to_string());
                    }
                });
            }
        }
        if folded {
            let _ = writer.send("You have folded\n".to_string());
        }
        if let Some(winner) = winner_str {
            let _ = writer.send(format!("Winner: {}\n", winner));
        }
        if round_ended {
            let _ = writer.send("Betting round ended, next round started\n".to_string());
        }
    } else {
        let _ = writer.send("You are not in a game\n".to_string());
    }
}