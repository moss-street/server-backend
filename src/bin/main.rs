use std::sync::Arc;

use anyhow::Result;
use moss_street_libs::{
    db::manager::DBManager,
    http::{dependencies::ServerDependencies, server::Server},
};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

#[tokio::main]
async fn main() -> Result<()> {
    let manager = SqliteConnectionManager::file("local.db");
    let pool = Pool::new(manager)?;
    let db_manager = Arc::new(DBManager::new(pool));

    let dependencies = ServerDependencies::new(db_manager);

    let ip = "127.0.0.1:6969";
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
