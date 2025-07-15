use crate::models::*;
use tokio::sync::mpsc::UnboundedSender;
use std::sync::{Arc, Mutex};

/// Sends the current game state to the user, including their cards, pot, community cards, and turn info.
pub async fn send_game_state(state: &Arc<Mutex<ServerState>>, user_id: &Option<UserId>, writer: &UnboundedSender<String>, _next_player: Option<String>, round_ended: bool, winner: Option<String>) {
    let (cards, pot, comm_cards, current_player, folded, _winner_str) = {
        let state = state.lock().unwrap();
        let mut cards = None;
        let mut pot = 0.0;
        let mut comm_cards = vec![];
        let mut current_player = None;
        let mut folded = false;
        let _winner_str: Option<String> = None;
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
        if let Some(winner) = _winner_str {
            let _ = writer.send(format!("Winner: {}\n", winner));
        }
        if round_ended {
            let _ = writer.send("Betting round ended, next round started\n".to_string());
        }
    } else {
        let _ = writer.send("You are not in a game\n".to_string());
    }
} 