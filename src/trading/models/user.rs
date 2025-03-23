use crate::db::models as db_models;
use std::collections::HashMap;

use tokio::sync::Mutex;

pub struct User {
    pub id: i32,
    pub email: String,
    pub ledger: HashMap<String, Wallet>,
}

impl From<crate::db::models::user::User> for User {
    fn from(value: db_models::user::User) -> User {
        // init with some default wallets for now
        // TODO: REMOVE THESE ONCE WE HAVE A BETTER SYSTEM OF CREATING WALLETS FOR USERS
        let ledger = HashMap::from_iter(
            vec![
                ("USD".into(), Wallet::new("USD")),
                ("BTC".into(), Wallet::new("BTC")),
                ("ETH".into(), Wallet::new("ETH")),
            ]
            .into_iter(),
        );
        User {
            id: value.id.unwrap(),
            email: value.email,
            ledger,
        }
    }
}

pub struct Wallet {
    symbol: String,
    quanity: Mutex<f64>,
}

pub enum UpdateIndicator {
    Add,
    Subtract,
}

impl Wallet {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            // TODO: REMOVE THE DEFAULT 50 HERE. THIS IS JUST HERE FOR TESTING RIGHT NOW
            quanity: Mutex::new(50.0),
        }
    }

    pub async fn update_balance(&self, ui: UpdateIndicator, amt: f64) {
        let mut guard = self.quanity.lock().await;
        match ui {
            UpdateIndicator::Add => *guard += amt,
            UpdateIndicator::Subtract => *guard -= amt,
        };
    }
}

impl std::hash::Hash for Wallet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
    }
}

impl std::cmp::PartialEq for Wallet {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
    }
}
// Necessary to keep inside a hashset
impl std::cmp::Eq for Wallet {}
