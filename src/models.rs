use std::collections::{HashMap, HashSet};
use crate::game::Game;
use tokio::sync::mpsc::UnboundedSender;

/// Unique identifier for a user
pub type UserId = String;
/// Unique identifier for a table
pub type TableId = String;

/// Represents a user connected to the server
#[derive(Debug)]
pub struct User {
    /// User's name (unique)
    pub name: String,
    /// User's current balance
    pub balance: f64,
    /// Table the user is currently sitting at (if any)
    pub table: Option<TableId>,
}

/// Represents a poker table
#[derive(Debug)]
pub struct Table {
    /// Table's unique identifier
    pub id: TableId,
    /// Set of user IDs of players at the table
    pub players: HashSet<UserId>,
    /// The current game at the table (if any)
    pub game: Option<Game>,
}

/// Global server state, shared between all connections
#[derive(Debug, Default)]
pub struct ServerState {
    /// All registered users (by user ID)
    pub users: HashMap<UserId, User>,
    /// All tables (by table ID)
    pub tables: HashMap<TableId, Table>,
    /// Channels for sending messages to users (by user ID)
    pub writers: HashMap<UserId, UnboundedSender<String>>,
}
