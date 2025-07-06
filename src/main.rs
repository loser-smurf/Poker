mod card;
mod deck;
mod player;
mod game;

fn main() {
    let mut deck = deck::Deck::new_shuffled();
    
    // Draw a card and print it
    if let Some(card) = deck.draw() {
        println!("Drew card: {:?}", card);
    } else {
        println!("No cards left in deck!");
    }

    let player1 = player::Player::new("John".to_string(), 100.0);
    let player2 = player::Player::new("Alice".to_string(), 100.0);
    println!("Player 1: {:?}", player1);
    println!("Player 2: {:?}", player2);

    let mut game = game::Game::new(5.0, 10.0); // small blind: 5, big blind: 10
    println!("Game: {:?}", game);

    game.add_player("John".to_string(), 100.0);
    game.add_player("Alice".to_string(), 100.0);
    println!("Game after adding players: {:?}", game);

    match game.start_new_hand() {
        Ok(_) => {
            println!("New hand started successfully!");
            println!("Pot: ${}", game.get_pot());
            println!("Community cards: {:?}", game.get_community_cards());
            println!("Active players: {}", game.get_active_players().len());
        }
        Err(e) => println!("Error starting new hand: {}", e),
    }
    

}
