use crate::card::Card;

#[derive(Debug, Clone)]
pub struct Player {
    pub name: String,
    pub balance: f64,
    pub hole_cards: Vec<Card>,
    pub hand_strength: f64,
    pub chips_in_play: f64,
    pub state: PlayerState,
    pub action: Option<PlayerAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerAction {
    Fold,
    Check,
    Call,
    Raise(f64),
    AllIn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerState {
    Active,
    Folded,
    AllIn,
    SittingOut,
}

impl Player {
    pub fn new(name: String, balance: f64) -> Self {
        Self {
            name,
            balance,
            hole_cards: Vec::new(),
            hand_strength: 0.0,
            chips_in_play: 0.0,
            state: PlayerState::Active,
            action: None,
        }
    }

    pub fn add_card(&mut self, card: Card) {
        self.hole_cards.push(card);
    }

    pub fn clear_cards(&mut self) {
        self.hole_cards.clear();
    }

    pub fn bet(&mut self, amount: f64) -> Result<f64, String> {
        if amount > self.balance {
            return Err("Insufficient funds".to_string());
        }
        if amount <= 0.0 {
            return Err("Bet amount must be positive".to_string());
        }
        
        self.balance -= amount;
        self.chips_in_play += amount;
        Ok(amount)
    }

    pub fn fold(&mut self) {
        self.state = PlayerState::Folded;
        self.action = Some(PlayerAction::Fold);
    }

    pub fn check(&mut self) {
        self.action = Some(PlayerAction::Check);
    }

    pub fn call(&mut self, amount: f64) -> Result<f64, String> {
        self.bet(amount)?;
        self.action = Some(PlayerAction::Call);
        Ok(amount)
    }

    pub fn raise(&mut self, amount: f64) -> Result<f64, String> {
        self.bet(amount)?;
        self.action = Some(PlayerAction::Raise(amount));
        Ok(amount)
    }

    pub fn all_in(&mut self) -> f64 {
        let amount = self.balance;
        self.balance = 0.0;
        self.chips_in_play += amount;
        self.state = PlayerState::AllIn;
        self.action = Some(PlayerAction::AllIn);
        amount
    }

    pub fn collect_winnings(&mut self, amount: f64) {
        self.balance += amount;
        self.chips_in_play = 0.0;
    }

    pub fn reset_for_new_hand(&mut self) {
        self.clear_cards();
        self.hand_strength = 0.0;
        self.chips_in_play = 0.0;
        self.state = PlayerState::Active;
        self.action = None;
    }
}