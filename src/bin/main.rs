use anyhow::Result;
use moss_street_libs::db::{DBManager, DatabaseImpl};

#[tokio::main]
async fn main() -> Result<()> {
    let connection = rusqlite::Connection::open("database.db")?;
    let dbmanager = DBManager::new(connection);

    dbmanager.create_table("users").await?;

    dbmanager.load_data_to_db("users", "lantz").await?;

    println!("Hello, world!");
    Ok(())
}
