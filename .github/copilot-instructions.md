# Server Backend Copilot Instructions

## Commands

`protoc` is required because `moss-street-api-models` generates gRPC types.
Install it with `brew install protobuf` on macOS.

```bash
cargo build
cargo test
cargo test trading::trade_engine::tests::sell_matches_oldest_equal_price_buy_order_first
cargo +nightly fmt -- --check
cargo clippy -- -D warnings
cargo +nightly fmt
cargo run --bin backend -- --ip 127.0.0.1 --port 8080 --database-uri local.db
```

Use `RUST_LOG='info,moss_street_libs=debug,backend=debug'` to explicitly
configure logging. The server defaults to application debug logs and dependency
info logs. Run `pre-commit install` to enable the formatter and Clippy hooks.

## Architecture

This is a Rust/Tokio gRPC backend. `src/bin/main.rs` initializes SQLite schema,
the connection pool, sessions, and `http::server::Server`. `AuthService` is
public; `TradeServiceImpl` requires the lowercase `auth` metadata header.
Authentication inserts the database `User` into tonic request extensions.

Markets are in-memory and keyed by direction-independent `SwapPair`; only
USD/BTC and USD/ETH are registered. `Market` wraps `TradeEngine`, which applies
reciprocal-price matching and price-time priority. `AssetLedger` persists
`available`/`reserved` balances and order reservations in SQLite.

New users start without funded assets. `AddFunds` is the only operation that
credits available balances; it creates the requested asset account if needed.
Trade reservation creates a zero-balance destination account only when needed
for settlement. Account funding, auth, reservation, submission, and
cancellation lifecycle events must remain logged with `tracing`.

`TradeStateStore` and order books are intentionally in memory today, so active
orders and trade lookup state are lost after restart. Startup refunds their
orphaned SQLite reservations and seeds new trade IDs above persisted reservation
IDs. Persisting and restoring those objects remains a tracked follow-up.

## Invariants

* Keep market mutation and settlement coupled through
  `submit_order_with_settlement` and `cancel_order_with_settlement`. Replace
  the live engine only after the ledger callback succeeds.
* Trade lifecycle ordering is validate market/request, initialize accounts,
  reserve source funds, submit and settle, refund unfilled non-resting
  remainders, then store trade state. Refund after any submission failure.
* `AssetLedger` is the source of truth for funds. Do not use the legacy
  in-memory `trading::models::user::Wallet` to bypass it.
* Use `SwapPair::new(source, destination)` for market lookup. Matching derives
  direction from the source symbol; inspect both source quantities on `Fill`.
* Best price wins; equal prices use the oldest (lowest order ID) first.
* Service tests authenticate by inserting a database `User` into request
  extensions. Integration scenarios belong in `trading/integration_harness.rs`.
* Add each persisted schema initializer to `main.rs`; ledger schema is
  initialized by `AssetLedger::initialize_database`.
