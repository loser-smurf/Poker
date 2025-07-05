mod card;
mod deck;
mod player;

fn main() {
    let mut deck = deck::Deck::new_shuffled();
    
    // Draw a card and print it
    if let Some(card) = deck.draw() {
        println!("Drew card: {:?}", card);
    } else {
        println!("No cards left in deck!");
    }
}
