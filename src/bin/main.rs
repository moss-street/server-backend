use std::sync::Arc;

use anyhow::Result;
use moss_street_libs::{
    db::{manager::DBManager, models::user::User},
    http::{dependencies::ServerDependencies, server::Server},
    session::manager::SessionManager,
};

use diesel::r2d2::{ConnectionManager, Pool};

#[tokio::main]
async fn main() -> Result<()> {
    let manager = ConnectionManager::new("local.db");
    let pool = Pool::new(manager)?;

    let db_manager = Arc::new(DBManager::new(pool));

    let Some(mut connection) = db_manager.connection_pool.try_get() else {
        return Err(anyhow::anyhow!("bad connection"));
    };

    let _ = User::initialize_database(&mut connection);

    let session_manager = Arc::new(SessionManager::default());

    let dependencies = ServerDependencies::new(db_manager, session_manager);

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
