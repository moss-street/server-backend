use anyhow::anyhow;
use anyhow::Ok;
use anyhow::Result;
use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use rusqlite::Row;

#[async_trait]
pub trait DatabaseImpl {
    async fn write_to_table<T: TableImpl + Send + Sync>(&self, obj: &T) -> Result<()>;
    async fn execute_db_query(&self, query: String) -> Result<()>;
    async fn db_lookup<T: TableImpl + Send>(&self, id: i32) -> Result<T>;
    async fn create_table<T: TableImpl + Send + Sync>(&self) -> Result<()>;
    async fn get_row<T: TableImpl + Send + Sync>(
        &self,
        fields: Vec<(&str, &str)>,
    ) -> Result<Vec<T>>;
}

#[derive(Debug)]
pub struct DBManager {
    pub connection_pool: Pool<SqliteConnectionManager>,
}

impl DBManager {
    pub fn new(connection_pool: Pool<SqliteConnectionManager>) -> Self {
        Self { connection_pool }
    }
}

// pub async fn lookup_user(manager : DBManager, id : i32) -> Result<User>{
//     return manager.db_lookup::<User>(id).await
// }

// pub async fn lookup_stock(manager : DBManager, id : i32) -> Result<Stock>{
//     return manager.db_lookup::<Stock>(id).await
// }

#[async_trait]
impl DatabaseImpl for DBManager {
    async fn execute_db_query(&self, query: String) -> Result<()> {
        if let Some(conn) = self.connection_pool.try_get() {
            conn.execute(query.as_str(), params![])?; // Execute the query
            Ok(())
        } else {
            Err(anyhow!("No available connection compadre"))
        }
    }

    async fn write_to_table<T: TableImpl + Send + Sync>(&self, obj: &T) -> Result<()> {
        let query: String = obj.generate_db_load_query();
        self.execute_db_query(query).await?;
        Ok(())
    }

    async fn db_lookup<T: TableImpl + Send>(&self, id: i32) -> Result<T> {
        if let Some(conn) = self.connection_pool.try_get() {
            let result: std::result::Result<T, _> = conn.query_row(
                T::generate_db_lookup_query(id).as_str(),
                [],
                T::deserialize_query_result,
            );
            Ok(result?)
        } else {
            Err(anyhow!("No available connection compadre"))
        }
    }
    async fn create_table<T: TableImpl + Send + Sync>(&self) -> Result<()> {
        let result = self.execute_db_query(T::create_table_query()).await;
        Ok(result?)
    }

    async fn get_row<T: TableImpl + Send + Sync>(
        &self,
        fields: Vec<(&str, &str)>,
    ) -> Result<Vec<T>> {
        // This monstrosity of a line loops through each tuple of field,value in the vector and formats them
        // As "field = "value"", and also joins each with an " and " inbetween for a nice sql query
        let where_clause = fields
            .iter()
            .map(|(col, value)| format!(r#"{} = "{}""#, col, value))
            .collect::<Vec<String>>()
            .join(" and ");

        let query = format!(
            "SELECT * FROM {} where {}",
            T::get_table_name(),
            where_clause
        );
        println!("Fucking rust : {}", query);

        if let Some(conn) = self.connection_pool.try_get() {
            let mut prepped_query = conn.prepare(&query)?;

            let fuck_im_so_good = prepped_query
                .query_map([], |row| T::deserialize_query_result(row))?
                .filter_map(|item| item.ok());

            let results = fuck_im_so_good.collect();
            Ok(results)
        } else {
            Err(anyhow!("No available connection compadre"))
        }
    }
}

pub trait TableImpl {
    fn create_table_query() -> String;
    fn generate_db_load_query(&self) -> String;
    fn generate_db_lookup_query(id: i32) -> String;
    fn deserialize_query_result(result: &Row) -> Result<Self, rusqlite::Error>
    where
        Self: Sized;
    fn get_table_name() -> String;
}
