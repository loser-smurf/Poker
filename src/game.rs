use crate::card::Card;
use crate::deck::Deck;
use crate::player::{Player, PlayerAction, PlayerState};

#[derive(Debug, Clone, PartialEq)]
pub enum BettingRound {
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
}

#[derive(Debug, Clone)]
pub struct Game {
    pub players: Vec<Player>,
    pub deck: Deck,
    pub current_player: usize,
    pub current_round: BettingRound,
    pub community_cards: Vec<Card>,
    pub pot: f64,
    pub current_bet: f64,
    pub dealer_position: usize,
    pub small_blind: f64,
    pub big_blind: f64,
    pub active_players: Vec<usize>,
}

impl Game {
    pub fn new(small_blind: f64, big_blind: f64) -> Self {
        Self {
            players: Vec::new(),
            deck: Deck::new_shuffled(),
            current_player: 0,
            current_round: BettingRound::PreFlop,
            community_cards: Vec::new(),
            pot: 0.0,
            current_bet: 0.0,
            dealer_position: 0,
            small_blind,
            big_blind,
            active_players: Vec::new(),
        }
    }

    pub fn add_player(&mut self, name: String, balance: f64) {
        let player = Player::new(name, balance);
        self.players.push(player);
    }

    pub fn start_new_hand(&mut self) -> Result<(), String> {
        if self.players.len() < 2 {
            return Err("Need at least 2 players to start a hand".to_string());
        }
        
        // Reset game state
        self.deck = Deck::new_shuffled();
        self.community_cards.clear();
        self.pot = 0.0;
        self.current_bet = 0.0;
        self.current_round = BettingRound::PreFlop;
        
        // Reset all players
        for player in &mut self.players {
            player.reset_for_new_hand();
        }
        
        // Move dealer button
        self.dealer_position = (self.dealer_position + 1) % self.players.len();
        
        // Post blinds
        self.post_blinds();
        
        // Deal hole cards
        self.deal_hole_cards();
        
        // Set active players
        self.update_active_players();
        
        // Set first player to act (after big blind)
        self.current_player = (self.dealer_position + 3) % self.players.len();
        
        Ok(())
    }

    fn post_blinds(&mut self) {
        let small_blind_pos = (self.dealer_position + 1) % self.players.len();
        let big_blind_pos = (self.dealer_position + 2) % self.players.len();
        
        // Post small blind
        if let Some(player) = self.players.get_mut(small_blind_pos) {
            if let Ok(_) = player.bet(self.small_blind) {
                self.pot += self.small_blind;
            }
        }
        
        // Post big blind
        if let Some(player) = self.players.get_mut(big_blind_pos) {
            if let Ok(_) = player.bet(self.big_blind) {
                self.pot += self.big_blind;
                self.current_bet = self.big_blind;
            }
        }
    }

    fn deal_hole_cards(&mut self) {
        // Deal 2 cards to each player
        for _ in 0..2 {
            for player in &mut self.players {
                if let Some(card) = self.deck.draw() {
                    player.add_card(card);
                }
            }
        }
    }

    fn update_active_players(&mut self) {
        self.active_players.clear();
        for (i, player) in self.players.iter().enumerate() {
            if player.state == PlayerState::Active || player.state == PlayerState::AllIn {
                self.active_players.push(i);
            }
        }
    }

    pub fn deal_flop(&mut self) {
        if self.current_round == BettingRound::PreFlop {
            // Burn one card
            self.deck.draw();
            
            // Deal 3 community cards
            for _ in 0..3 {
                if let Some(card) = self.deck.draw() {
                    self.community_cards.push(card);
                }
            }
            
            self.current_round = BettingRound::Flop;
            self.current_bet = 0.0;
            self.reset_player_actions();
        }
    }

    pub fn deal_turn(&mut self) {
        if self.current_round == BettingRound::Flop {
            // Burn one card
            self.deck.draw();
            
            // Deal 1 community card
            if let Some(card) = self.deck.draw() {
                self.community_cards.push(card);
            }
            
            self.current_round = BettingRound::Turn;
            self.current_bet = 0.0;
            self.reset_player_actions();
        }
    }

    pub fn deal_river(&mut self) {
        if self.current_round == BettingRound::Turn {
            // Burn one card
            self.deck.draw();
            
            // Deal 1 community card
            if let Some(card) = self.deck.draw() {
                self.community_cards.push(card);
            }
            
            self.current_round = BettingRound::River;
            self.current_bet = 0.0;
            self.reset_player_actions();
        }
    }

    fn reset_player_actions(&mut self) {
        for player in &mut self.players {
            player.action = None;
        }
    }

    pub fn player_action(&mut self, player_index: usize, action: PlayerAction) -> Result<(), String> {
        if player_index >= self.players.len() {
            return Err("Invalid player index".to_string());
        }
        
        let player = &mut self.players[player_index];
        
        match action {
            PlayerAction::Fold => {
                player.fold();
                self.update_active_players();
            }
            PlayerAction::Check => {
                if self.current_bet > player.chips_in_play {
                    return Err("Cannot check when there's a bet to call".to_string());
                }
                player.check();
            }
            PlayerAction::Call => {
                let call_amount = self.current_bet - player.chips_in_play;
                if call_amount > 0.0 {
                    player.call(call_amount)?;
                    self.pot += call_amount;
                } else {
                    player.check();
                }
            }
            PlayerAction::Raise(amount) => {
                let total_needed = self.current_bet + amount;
                player.raise(amount)?;
                self.pot += amount;
                self.current_bet = total_needed;
                self.reset_other_player_actions(player_index);
            }
            PlayerAction::AllIn => {
                let amount = player.all_in();
                self.pot += amount;
                if player.chips_in_play > self.current_bet {
                    self.current_bet = player.chips_in_play;
                    self.reset_other_player_actions(player_index);
                }
            }
        }
        
        Ok(())
    }

    fn reset_other_player_actions(&mut self, current_player: usize) {
        for (i, player) in self.players.iter_mut().enumerate() {
            if i != current_player && player.state == PlayerState::Active {
                player.action = None;
            }
        }
    }

    pub fn next_player(&mut self) {
        loop {
            self.current_player = (self.current_player + 1) % self.players.len();
            if self.players[self.current_player].state == PlayerState::Active {
                break;
            }
        }
    }

    pub fn is_betting_round_complete(&self) -> bool {
        let active_players = self.players.iter()
            .filter(|p| p.state == PlayerState::Active)
            .collect::<Vec<_>>();
        
        if active_players.len() <= 1 {
            return true;
        }
        
        // Check if all active players have acted and bets are equal
        let first_bet = active_players[0].chips_in_play;
        active_players.iter().all(|p| {
            p.action.is_some() && p.chips_in_play == first_bet
        })
    }

    pub fn get_pot(&self) -> f64 {
        self.pot
    }

    pub fn get_community_cards(&self) -> &[Card] {
        &self.community_cards
    }

    pub fn get_current_player(&self) -> Option<&Player> {
        self.players.get(self.current_player)
    }

    pub fn get_active_players(&self) -> Vec<&Player> {
        self.players.iter()
            .filter(|p| p.state == PlayerState::Active || p.state == PlayerState::AllIn)
            .collect()
    }
}