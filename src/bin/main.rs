use std::{net::Ipv4Addr, sync::Arc};

use anyhow::Result;
use clap::Parser;
use moss_street_libs::{
    db::{manager::DBManager, models::user::User},
    http::{dependencies::ServerDependencies, server::Server},
    session::manager::SessionManager,
};

use diesel::r2d2::{ConnectionManager, Pool};

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
    let args = Args::parse();

    let manager = ConnectionManager::new(args.database_uri);
    let pool = Pool::new(manager)?;

    let db_manager = Arc::new(DBManager::new(pool));

    let Some(mut connection) = db_manager.connection_pool.try_get() else {
        return Err(anyhow::anyhow!("bad connection"));
    };

    let _ = User::initialize_database(&mut connection);

    let session_manager = Arc::new(SessionManager::default());

    let dependencies = ServerDependencies::new(db_manager, session_manager);

    let ip = format!("{}:{}", args.ip, args.port);
    let addr = ip.parse()?;

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
