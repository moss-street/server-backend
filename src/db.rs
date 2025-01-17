use super::server::dependencies::ServerDependencies;
use anyhow::anyhow;
use anyhow::Ok;
use anyhow::Result;
use async_trait::async_trait;
use rusqlite::params;
use rusqlite::Row;

#[async_trait]
pub trait DatabaseImpl {
    async fn write_to_table<T: TableImpl + Send>(&self, obj: T) -> Result<()>;
    async fn execute_db_query(&self, query: String) -> Result<()>;
    async fn db_lookup<T: TableImpl + Send>(&self, id: i32) -> Result<T>;
}

pub struct DBManager {
    pub dependencies: ServerDependencies,
}

impl DBManager {
    pub fn new(dependencies: ServerDependencies) -> Self {
        Self { dependencies }
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
        if let Some(conn) = self.dependencies.get_connection() {
            conn.execute(query.as_str(), params![])?; // Execute the query
            Ok(())
        } else {
            Err(anyhow!("No available connection compadre"))
        }
    }

    async fn write_to_table<T: TableImpl + Send>(&self, obj: T) -> Result<()> {
        let query: String = obj.generate_db_load_query();
        self.execute_db_query(query).await?;
        Ok(())
    }

    async fn db_lookup<T: TableImpl + Send>(&self, id: i32) -> Result<T> {
        if let Some(conn) = self.dependencies.get_connection() {
            let result: std::result::Result<T, _> = conn
                .query_row(T::generate_db_lookup_query(id).as_str(), [], T::deserialize_query_result);
            Ok(result?)
        } else {
            Err(anyhow!("No available connection compadre"))
        }
    }


}

pub trait TableImpl {
    fn generate_db_load_query(&self) -> String;
    fn generate_db_lookup_query(id: i32) -> String;
    fn deserialize_query_result(result : &Row) -> Result<Self, rusqlite::Error> where Self: Sized;
}

#[derive(Debug)]
pub struct User {
    id: i32,
    email: String,
    created_at: String, // Using String for simplicity, or use a DateTime library
}

impl TableImpl for User {
    fn generate_db_load_query(&self) -> String {
        format!(
            "INSERT INTO users (id, email, created_at) VALUES ({}, '{}', '{}')",
            self.id,
            self.email.replace("'", "''"), // Escape single quotes
            self.created_at
        )
    }

    fn generate_db_lookup_query(id: i32) -> String {
        todo!()
    }

    fn deserialize_query_result(result : &Row) -> Result<Self, rusqlite::Error>{
        todo!()
    }
}

pub struct Stock {
    id: i32,
    name: String,
}

impl TableImpl for Stock {
    fn generate_db_load_query(&self) -> String {
        todo!()
    }

    fn generate_db_lookup_query(id: i32) -> String {
        todo!()
    }

    fn deserialize_query_result(result : &Row) -> Result<Self, rusqlite::Error>{
        todo!()
    }
}
