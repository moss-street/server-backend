#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use diesel::{r2d2::ConnectionManager, sqlite::SqliteConnection};

    use rust_models::common::{
        create_trade_response::CreateTradeStatus,
        delete_trade_response::DeleteTradeStatus,
        get_trade_response::GetTradeStatus,
        trade_request::{TradeType, TransactionType},
        trade_service_server::TradeService,
        CreateTradeRequest, DeleteTradeRequest, GetTradeRequest, TradeId, TradeRequest,
    };

    use crate::{
        db::{manager::DBManager, models::user::User as DbUser},
        http::dependencies::ServerDependencies,
        services::trading::{TradeServiceImpl, UserTradeSubmissionGuard},
        session::manager::SessionManager,
        trading::backend::TradeBackend,
    };

    use crate::trading::{
        market::{Market, MarketProcessor},
        models::{
            market_order::MarketOrder,
            user::{User as TraderUser, Wallet},
        },
    };

    fn fake_trader_user(user_id: i32) -> TraderUser {
        TraderUser {
            _id: user_id,
            _email: format!("user-{user_id}@example.com"),
            ledger: HashMap::from([
                ("USD".to_string(), Wallet::new("USD")),
                ("BTC".to_string(), Wallet::new("BTC")),
            ]),
        }
    }

    fn limit_order(
        user_id: i32,
        source_symbol: &str,
        dest_symbol: &str,
        source_qty: f64,
        price: f64,
    ) -> MarketOrder {
        MarketOrder::new(
            TradeRequest {
                trade_type: TradeType::Limit as i32,
                transaction_type: TransactionType::Buy as i32,
                symbol_source: source_symbol.to_string(),
                symbol_dest: dest_symbol.to_string(),
                source_quantity: source_qty,
                price: Some(price),
                valid_from: None,
                valid_to: None,
            },
            fake_trader_user(user_id),
        )
    }

    fn db_user(user_id: i32) -> DbUser {
        DbUser {
            id: Some(user_id),
            email: format!("db-user-{user_id}@example.com"),
            password: "pw".to_string(),
            first_name: "first".to_string(),
            last_name: "last".to_string(),
        }
    }

    fn build_trade_service() -> TradeServiceImpl {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = diesel::r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("sqlite pool should build for tests");

        let dependencies = ServerDependencies::new(
            Arc::new(DBManager::new(pool)),
            Arc::new(SessionManager::default()),
        );

        let mut trade_backend = TradeBackend::new();
        trade_backend.add_market(Market::new("USD", "BTC"));

        TradeServiceImpl::new(
            dependencies,
            trade_backend,
            UserTradeSubmissionGuard::default(),
        )
    }

    fn lifecycle_trade_request(
        symbol_source: &str,
        symbol_dest: &str,
        qty: f64,
        price: f64,
    ) -> TradeRequest {
        TradeRequest {
            trade_type: TradeType::Limit as i32,
            transaction_type: TransactionType::Buy as i32,
            symbol_source: symbol_source.to_string(),
            symbol_dest: symbol_dest.to_string(),
            source_quantity: qty,
            price: Some(price),
            valid_from: None,
            valid_to: None,
        }
    }

    fn create_trade_request(
        user_id: i32,
        trade_request: TradeRequest,
    ) -> tonic::Request<CreateTradeRequest> {
        let mut request = tonic::Request::new(CreateTradeRequest {
            trade_request: Some(trade_request),
        });
        request.extensions_mut().insert(db_user(user_id));
        request
    }

    fn get_trade_request(user_id: i32, trade_id: i32) -> tonic::Request<GetTradeRequest> {
        let mut request = tonic::Request::new(GetTradeRequest {
            trade_id: Some(TradeId { trade_id }),
        });
        request.extensions_mut().insert(db_user(user_id));
        request
    }

    fn delete_trade_request(user_id: i32, trade_id: i32) -> tonic::Request<DeleteTradeRequest> {
        let mut request = tonic::Request::new(DeleteTradeRequest {
            trade_id: Some(TradeId { trade_id }),
        });
        request.extensions_mut().insert(db_user(user_id));
        request
    }

    #[test]
    fn harness_runs_trade_sequence_and_tracks_book_state() {
        let market = Market::new("USD", "BTC");

        // Step 1: Seller posts 10 USD @ 100 BTC/USD equivalent.
        market
            .send_order(limit_order(1, "USD", "BTC", 10.0, 100.0))
            .expect("sell order should be accepted");
        assert_eq!(market.book_depths(), (0, 1));

        // Step 2: Buyer posts non-crossing bid 3 BTC @ 90.
        market
            .send_order(limit_order(2, "BTC", "USD", 3.0, 90.0))
            .expect("non-crossing buy should be accepted");
        assert_eq!(market.book_depths(), (1, 1));

        // Step 3: Crossing buy at 100 fills 4 units against resting sell.
        let fills = market
            .send_order_and_collect_fills(limit_order(3, "BTC", "USD", 4.0, 100.0))
            .expect("crossing buy should execute");
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 4.0);
        assert_eq!(fills[0].price, 100.0);
        assert_eq!(market.book_depths(), (1, 1));

        // Step 4: Crossing buy at 110 consumes the remaining 6 units.
        let fills = market
            .send_order_and_collect_fills(limit_order(4, "BTC", "USD", 6.0, 110.0))
            .expect("crossing buy should execute remaining sell quantity");
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity, 6.0);
        assert_eq!(fills[0].price, 100.0);

        // The non-crossing bid from step 2 should still be resting.
        assert_eq!(market.book_depths(), (1, 0));
    }

    #[test]
    fn harness_rejects_symbols_outside_market_pair() {
        let market = Market::new("USD", "BTC");

        let result = market.send_order(limit_order(9, "ETH", "USD", 1.0, 100.0));

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn harness_tracks_trade_lifecycle_through_service() {
        let service = build_trade_service();

        let create = service
            .create_trade(create_trade_request(
                77,
                lifecycle_trade_request("USD", "BTC", 4.0, 100.0),
            ))
            .await
            .expect("create_trade should succeed")
            .into_inner();

        assert_eq!(create.status, CreateTradeStatus::Ok as i32);
        let trade_id = create
            .trade_id
            .expect("trade id should be present after create")
            .trade_id;

        let get_before_delete = service
            .get_trade(get_trade_request(77, trade_id))
            .await
            .expect("get_trade should succeed")
            .into_inner();
        assert_eq!(get_before_delete.status, GetTradeStatus::Ok as i32);
        assert!(get_before_delete.trade_request.is_some());

        let delete = service
            .delete_trade(delete_trade_request(77, trade_id))
            .await
            .expect("delete_trade should succeed")
            .into_inner();
        assert_eq!(delete.status, DeleteTradeStatus::Ok as i32);

        let get_after_delete = service
            .get_trade(get_trade_request(77, trade_id))
            .await
            .expect("get_trade after delete should succeed")
            .into_inner();
        assert_eq!(get_after_delete.status, GetTradeStatus::NotFound as i32);
        assert!(get_after_delete.trade_request.is_none());
    }
}
