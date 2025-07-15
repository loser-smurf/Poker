use crate::models::*;
use tokio::sync::mpsc::UnboundedSender;
use std::sync::{Arc, Mutex};
use crate::utils::send_game_state;

/// Handles user registration. Registers a new user if the name is not taken.
pub fn handle_register(name: &str, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>, user_id: &mut Option<UserId>) {
    let mut already_exists = false;
    {
        let mut state = state.lock().unwrap();
        if state.users.contains_key(name) {
            already_exists = true;
        } else {
            state.users.insert(name.to_string(), User { name: name.to_string(), balance: 100.0, table: None });
            *user_id = Some(name.to_string());
            state.writers.insert(name.to_string(), tx.clone());
        }
    }
    if already_exists {
        let _ = tx.send("Username already taken\n".to_string());
    } else {
        let _ = tx.send("Registered successfully. Your balance: 100\n".to_string());
    }
}

/// Handles table creation. Creates a new table if the name is not taken.
pub fn handle_create_table(table: &str, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let mut already_exists = false;
    {
        let mut state = state.lock().unwrap();
        if state.tables.contains_key(table) {
            already_exists = true;
        } else {
            state.tables.insert(table.to_string(), Table { id: table.to_string(), players: std::collections::HashSet::new(), game: None });
        }
    }
    if already_exists {
        let _ = tx.send("Table already exists\n".to_string());
    } else {
        let _ = tx.send("Table created\n".to_string());
    }
}

/// Handles joining a table. Adds the user to the table if it exists.
pub fn handle_join_table(user_id: &Option<UserId>, table: &str, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    if let Some(uid) = user_id {
        let table_key = table.to_string();
        let user_key = uid.clone().to_string();
        let mut joined = false;
        {
            let mut state = state.lock().unwrap();
            if let Some(table_obj) = state.tables.get_mut(&table_key) {
                table_obj.players.insert(user_key.clone());
                joined = true;
            }
            if let Some(user) = state.users.get_mut(&user_key) {
                user.table = Some(table_key.clone());
            }
        }
        if joined {
            let _ = tx.send("Joined table\n".to_string());
        } else {
            let _ = tx.send("Table not found\n".to_string());
        }
    } else {
        let _ = tx.send("You must register first\n".to_string());
    }
}

/// Lists all available tables.
pub fn handle_list_tables(state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let list = {
        let state = state.lock().unwrap();
        state.tables.keys().cloned().collect::<Vec<_>>().join(", ")
    };
    let _ = tx.send(format!("Tables: {}\n", list));
}

/// Shows the current user's info and table state.
pub async fn handle_show(user_id: &Option<UserId>, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let (user_name, user_balance, table_info) = {
        let state = state.lock().unwrap();
        if let Some(uid) = user_id {
            if let Some(user) = state.users.get(uid) {
                let user_name = user.name.clone();
                let user_balance = user.balance;
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
                (user_name, user_balance, table_info)
            } else {
                (String::new(), 0.0, String::new())
            }
        } else {
            (String::new(), 0.0, String::new())
        }
    };
    if !user_name.is_empty() {
        let _ = tx.send(format!("You: {} | Balance: {}\n", user_name, user_balance));
        if !table_info.is_empty() {
            let _ = tx.send(table_info);
        }
    }
}

/// Handles the quit command. Sends a goodbye message.
pub async fn handle_quit(tx: &UnboundedSender<String>) {
    let _ = tx.send("Bye!\n".to_string());
}

/// Handles a bet action from the user.
pub async fn handle_bet(user_id: &Option<UserId>, amount: f64, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let mut result = String::new();
    let mut next_player_name = None;
    let mut round_ended = false;
    let mut winner_info = None;
    {
        let mut state = state.lock().unwrap();
        if let Some(uid) = user_id {
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
    send_game_state(state, user_id, tx, next_player_name, round_ended, winner_info).await;
}

/// Handles a call action from the user.
pub async fn handle_call(user_id: &Option<UserId>, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let mut result = String::new();
    let mut next_player_name = None;
    let mut round_ended = false;
    let mut winner_info = None;
    {
        let mut state = state.lock().unwrap();
        if let Some(uid) = user_id {
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
    send_game_state(state, user_id, tx, next_player_name, round_ended, winner_info).await;
}

/// Handles a check action from the user.
pub async fn handle_check(user_id: &Option<UserId>, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let mut result = String::new();
    let mut next_player_name = None;
    let mut round_ended = false;
    let mut winner_info = None;
    {
        let mut state = state.lock().unwrap();
        if let Some(uid) = user_id {
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
    send_game_state(state, user_id, tx, next_player_name, round_ended, winner_info).await;
}

/// Handles a fold action from the user.
pub async fn handle_fold(user_id: &Option<UserId>, state: &Arc<Mutex<ServerState>>, tx: &UnboundedSender<String>) {
    let mut result = String::new();
    let mut next_player_name = None;
    let mut round_ended = false;
    let mut winner_info = None;
    {
        let mut state = state.lock().unwrap();
        if let Some(uid) = user_id {
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
    send_game_state(state, user_id, tx, next_player_name, round_ended, winner_info).await;
}

/// Shows the current game state to the user.
pub async fn handle_show_state(state: &Arc<Mutex<ServerState>>, user_id: &Option<UserId>, tx: &UnboundedSender<String>) {
    send_game_state(state, user_id, tx, None, false, None).await;
}
