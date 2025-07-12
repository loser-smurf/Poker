mod card;
mod deck;
mod player;
mod game;
mod hand;

use std::io::{self, Write};
use crate::game::Game;
use crate::player::{PlayerAction, PlayerState};
use crate::hand::evaluate_hand;

fn main() {
    println!("=== Texas Hold'em Poker Game ===");
    
    let mut game = Game::new(5.0, 10.0); // Small blind: 5, Big blind: 10
    
    loop {
        println!("\n=== Main Menu ===");
        println!("1. Add player");
        println!("2. Start new hand");
        println!("3. Deal flop");
        println!("4. Deal turn");
        println!("5. Deal river");
        println!("6. Show current state");
        println!("7. Player action");
        println!("8. Evaluate hands");
        println!("9. Exit");
        
        print!("Choose option: ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        match input.trim() {
            "1" => add_player(&mut game),
            "2" => start_new_hand(&mut game),
            "3" => deal_flop(&mut game),
            "4" => deal_turn(&mut game),
            "5" => deal_river(&mut game),
            "6" => show_current_state(&game),
            "7" => player_action(&mut game),
            "8" => evaluate_hands(&game),
            "9" => {
                println!("Goodbye!");
                break;
            }
            _ => println!("Invalid option!"),
        }
    }
}

fn add_player(game: &mut Game) {
    print!("Enter player name: ");
    io::stdout().flush().unwrap();
    let mut name = String::new();
    io::stdin().read_line(&mut name).unwrap();
    let name = name.trim().to_string();
    
    print!("Enter starting balance: ");
    io::stdout().flush().unwrap();
    let mut balance = String::new();
    io::stdin().read_line(&mut balance).unwrap();
    let balance: f64 = balance.trim().parse().unwrap_or(100.0);
    
    game.add_player(name.clone(), balance);
    println!("Player {} added with balance ${}", name, balance);
}

fn start_new_hand(game: &mut Game) {
    match game.start_new_hand() {
        Ok(_) => {
            println!("New hand started!");
            println!("Pot: ${}", game.get_pot());
            println!("Community cards: {:?}", game.get_community_cards());
            println!("Active players: {}", game.get_active_players().len());
            
            // Show hole cards for each player
            for (i, player) in game.players.iter().enumerate() {
                println!("Player {} ({}): {:?}", i, player.name, player.hole_cards);
            }
        }
        Err(e) => println!("Error starting new hand: {}", e),
    }
}

fn deal_flop(game: &mut Game) {
    game.deal_flop();
    println!("Flop dealt!");
    println!("Community cards: {:?}", game.get_community_cards());
    println!("Current round: {:?}", game.current_round);
}

fn deal_turn(game: &mut Game) {
    game.deal_turn();
    println!("Turn dealt!");
    println!("Community cards: {:?}", game.get_community_cards());
    println!("Current round: {:?}", game.current_round);
}

fn deal_river(game: &mut Game) {
    game.deal_river();
    println!("River dealt!");
    println!("Community cards: {:?}", game.get_community_cards());
    println!("Current round: {:?}", game.current_round);
}

fn show_current_state(game: &Game) {
    println!("\n=== Current Game State ===");
    println!("Pot: ${}", game.get_pot());
    println!("Current bet: ${}", game.current_bet);
    println!("Current round: {:?}", game.current_round);
    println!("Community cards: {:?}", game.get_community_cards());
    println!("Dealer position: {}", game.dealer_position);
    
    println!("\nPlayers:");
    for (i, player) in game.players.iter().enumerate() {
        println!("  {}: {} (${}) - State: {:?}, Action: {:?}", 
                i, player.name, player.balance, player.state, player.action);
        if !player.hole_cards.is_empty() {
            println!("    Hole cards: {:?}", player.hole_cards);
        }
    }
    
    if let Some(current_player) = game.get_current_player() {
        println!("\nCurrent player to act: {} (${})", 
                current_player.name, current_player.balance);
    }
}

fn player_action(game: &mut Game) {
    if game.players.is_empty() {
        println!("No players in the game!");
        return;
    }
    
    print!("Enter player index: ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let player_index: usize = input.trim().parse().unwrap_or(0);
    
    if player_index >= game.players.len() {
        println!("Invalid player index!");
        return;
    }
    
    println!("Actions: 1=Fold, 2=Check, 3=Call, 4=Raise, 5=AllIn");
    print!("Choose action: ");
    io::stdout().flush().unwrap();
    
    let mut action_input = String::new();
    io::stdin().read_line(&mut action_input).unwrap();
    
    let action = match action_input.trim() {
        "1" => PlayerAction::Fold,
        "2" => PlayerAction::Check,
        "3" => PlayerAction::Call,
        "4" => {
            print!("Enter raise amount: ");
            io::stdout().flush().unwrap();
            let mut amount_input = String::new();
            io::stdin().read_line(&mut amount_input).unwrap();
            let amount: f64 = amount_input.trim().parse().unwrap_or(0.0);
            PlayerAction::Raise(amount)
        }
        "5" => PlayerAction::AllIn,
        _ => {
            println!("Invalid action!");
            return;
        }
    };
    
    match game.player_action(player_index, action) {
        Ok(_) => println!("Action executed successfully!"),
        Err(e) => println!("Error executing action: {}", e),
    }
}

fn evaluate_hands(game: &Game) {
    if game.community_cards.len() < 5 {
        println!("Need 5 community cards to evaluate hands!");
        return;
    }
    
    println!("\n=== Hand Evaluation ===");
    let mut player_hands = Vec::new();
    
    for (i, player) in game.players.iter().enumerate() {
        if player.state == PlayerState::Folded {
            println!("Player {} ({}): Folded", i, player.name);
            continue;
        }
        
        // Combine hole cards with community cards
        let mut all_cards = player.hole_cards.clone();
        all_cards.extend(game.community_cards.iter().cloned());
        
        if all_cards.len() >= 5 {
            let evaluated_hand = evaluate_hand(&all_cards);
            player_hands.push((i, player.name.clone(), evaluated_hand.clone()));
            println!("Player {} ({}): {:?} - {:?}", 
                    i, player.name, evaluated_hand.rank, evaluated_hand.cards);
        }
    }
    
    // Find winner
    if !player_hands.is_empty() {
        player_hands.sort_by(|a, b| b.2.cmp(&a.2));
        let winner = &player_hands[0];
        println!("\nWinner: Player {} ({}) with {:?}!", 
                winner.0, winner.1, winner.2.rank);
    }
}
