use anyhow::Result;
use rusqlite::Connection;
use std::future::Future;

pub trait DatabaseImpl {
    fn load_data_to_db(
        &self,
        table_name: impl Into<String>,
        email: impl Into<String>,
    ) -> impl Future<Output = Result<()>>;
    fn create_table(&self, table_name: impl Into<String>) -> impl Future<Output = Result<()>>;
    fn execute_query(&self, query: &str) -> impl Future<Output = Result<()>>;
}

impl DatabaseImpl for DBManager {
    async fn create_table(&self, table_name: impl Into<String>) -> Result<()> {
        let table_name = table_name.into();

        let create_table_query = format!(
            "CREATE TABLE IF NOT EXISTS {table_name} (
                email TEXT PRIMARY KEY
            )"
        );

        self.execute_query(create_table_query.as_str()).await?;

        Ok(())
    }

    async fn load_data_to_db(
        &self,
        table_name: impl Into<String>,
        email: impl Into<String>,
    ) -> Result<()> {
        // Create a table if it doesn't already exist
        let table_name = table_name.into();
        let email = email.into();
        let insert_query = format!(
            "
            INSERT INTO {table_name} (email)
            VALUES('{email}');
        "
        );

        self.execute_query(insert_query.as_str()).await?;

        Ok(())
    }

    async fn execute_query(&self, query: &str) -> Result<()> {
        self.connection.execute(query, [])?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DBManager {
    pub connection: Connection,
}

impl DBManager {
    pub fn new(connection: Connection) -> Self {
        DBManager { connection }
    }
}
