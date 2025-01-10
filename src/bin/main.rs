use anyhow::Result;
use moss_street_libs::{
    db::{DBManager, DatabaseImpl},
    server::Server,
};

#[tokio::main]
async fn main() -> Result<()> {
    let connection = rusqlite::Connection::open("database.db")?;
    let dbmanager = DBManager::new(connection);

    dbmanager.create_table("users").await?;

    let ip = "127.0.0.1:6969";
    let addr = ip.parse()?;

    let _ = Server::new(addr, dbmanager).await;
    loop {}

    Ok(())
}
