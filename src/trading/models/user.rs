use crate::db::models as db_models;
use std::collections::HashMap;

use rust_models::common::TradeRequest;
use tokio::sync::Mutex;

#[derive(Debug, Eq, PartialEq)]
pub struct User {
    pub _id: i32,
    pub _email: String,
    pub ledger: HashMap<String, Wallet>,
}

pub trait WalletOperations{
    async fn check_order_prereqs(&self, request: TradeRequest) -> Result<(), tonic::Status> ;
}

impl WalletOperations for User{
    async fn check_order_prereqs(&self, request: TradeRequest) -> Result<(), tonic::Status> {
        let source_wallet = self.ledger.get(&request.symbol_source).ok_or_else(|| {
            tonic::Status::not_found(format!(
                "Wallet {} not found for user",
                &request.symbol_source
            ))
        })?;
        let _dest_wallet = self.ledger.get(&request.symbol_dest).ok_or_else(|| {
            tonic::Status::not_found(format!(
                "Wallet {} not found for user",
                &request.symbol_dest
            ))
        })?;

        // Lock the mutex and check if the quantity is sufficient
        let source_balance = source_wallet.quanity.try_lock().map_err(|_| {
            tonic::Status::internal("Failed to lock source wallet quantity")
        })?;

        if *source_balance < request.source_quantity {
            tonic::Status::failed_precondition(format!(
                "Insufficient balance for {}: {} available, but {} was requested",
                &request.symbol_source, *source_balance, &request.source_quantity
            ));
        }
        Ok(())


    }

    
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
            _id: value.id.unwrap(),
            _email: value.email,
            ledger,
        }
    }
}

#[derive(Debug)]
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

    pub async fn check_sufficient_funds(&self, amt: f64) -> bool{
        let wallet_quantity  = self.quanity.lock().await;
        *wallet_quantity > amt
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
