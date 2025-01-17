use anyhow::Ok;
use chrono::{NaiveDateTime, Utc};
use async_trait::async_trait;
use rusqlite::{params, Connection};

#[async_trait]
pub trait DatabaseImpl {
    async fn load_obj_to_db<T: DatabaseStruct>(&self, obj: T) -> Result<(), Error>;
    async fn execute_db_query(&self, query : String) -> Result<(), Error>;
}

#[async_trait]
impl DatabaseImpl for DBManager {
    async fn execute_db_query(&self, query : String) -> Result<(), Error>{
        // Do the magic
        Ok(())
    }
    async fn load_obj_to_db(&self, obj: impl DatabaseStruct) -> Result<(), Error>{
        let query : String = obj.generate_db_load_query();
        self.execute_db_query(query);
        Ok(())
    }
}

pub struct DBManager {
    connection : Connection
}

pub trait DatabaseStruct {
    fn generate_db_load_query (&self) -> String;
}

#[derive(Debug)]
struct User {
    id: i32,
    email: String,
    created_at: NaiveDateTime,
}


impl DatabaseStruct for User{
    fn generate_db_load_query(&self) -> String {
        let query : String = format!("INSERT INTO users (id, email, created_at) VALUES ({}, {}, {})", self.id, self.email, self.created_at);

        return query;
    }
}
