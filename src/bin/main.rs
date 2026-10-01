use std::{net::Ipv4Addr, sync::Arc};

use anyhow::Result;
use clap::Parser;
use moss_street_libs::{
    db::{
        manager::DBManager,
        models::{stock::Stock, user::User, wallet::Wallet},
    },
    http::{dependencies::ServerDependencies, server::Server},
    ledger::AssetLedger,
    session::manager::SessionManager,
};

use diesel::r2d2::{ConnectionManager, Pool};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Server ip to start the server on
    #[arg(short, long, default_value = "127.0.0.1")]
    ip: Ipv4Addr,

    /// Server port to start the server on
    #[arg(short, long, default_value = "8080")]
    port: u32,

    /// Location of database or uri of database
    #[arg(short, long, default_value = "local.db")]
    database_uri: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,moss_street_libs=debug,backend=debug")),
        )
        .with_target(false)
        .init();

    let args = Args::parse();
    info!(
        database_uri = %args.database_uri,
        ip = %args.ip,
        port = args.port,
        "Initializing backend database"
    );

    let manager = ConnectionManager::new(args.database_uri.clone());
    let pool = Pool::new(manager)?;

    let db_manager = Arc::new(DBManager::new(pool));

    let Some(mut connection) = db_manager.connection_pool.try_get() else {
        return Err(anyhow::anyhow!("bad connection"));
    };

    User::initialize_database(&mut connection)?;
    Stock::initialize_database(&mut connection)?;
    Wallet::initialize_database(&mut connection)?;
    AssetLedger::initialize_database(&mut connection)?;

    let session_manager = Arc::new(SessionManager::default());

    let dependencies = ServerDependencies::new(db_manager, session_manager);

    let ip = format!("{}:{}", args.ip, args.port);
    let addr = ip.parse()?;
    info!(%addr, "Starting backend server");

    let server = Server::new(addr, dependencies).await;
    async move {
        server
            .server_handle
            .await
            .expect("Server handle paniced! Closing server");
    }
    .await;

    Ok(())
}
