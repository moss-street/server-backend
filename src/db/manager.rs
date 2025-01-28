use anyhow::{anyhow, Result};
use async_trait::async_trait;

use diesel::query_builder::{InsertStatement, QueryFragment, QueryId};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel::{query_dsl::methods::ExecuteDsl, sqlite::Sqlite, Table};
use diesel::{Insertable, RunQueryDsl};

#[async_trait]
pub trait DatabaseImpl {
    async fn query_row<T>(&self, fields: Vec<(&str, &str)>) -> Result<T>
    where
        T: Send + Sync + 'static;

    async fn query_rows<T>(&self, fields: Vec<(&str, &str)>) -> Result<Vec<T>>
    where
        T: Send + Sync + 'static;

    fn insert_row<T, U>(&self, table: T, obj: &U) -> Result<usize>
    where
        T: Table + Send + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>;

    async fn insert_rows<T>(&self, obj: &Vec<&T>) -> Result<()>
    where
        T: Send + Sync + 'static;
}

#[derive(Debug)]
pub struct DBManager {
    pub connection_pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl DBManager {
    pub fn new(connection_pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self { connection_pool }
    }
}

#[async_trait]
impl DatabaseImpl for DBManager {
    async fn query_row<T>(&self, fields: Vec<(&str, &str)>) -> Result<T>
    where
        T: Send + Sync + 'static,
    {
        todo!()
    }
    async fn query_rows<T>(&self, fields: Vec<(&str, &str)>) -> Result<Vec<T>>
    where
        T: Send + Sync + 'static,
    {
        todo!()
    }

    fn insert_row<T, U>(&self, table: T, obj: &U) -> Result<usize>
    where
        T: Table + Send + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>,
    {
        let Some(mut conn) = self.connection_pool.try_get() else {
            return Err(anyhow!("No available connection in connection pool!"));
        };

        diesel::insert_into(table)
            .values(obj.clone())
            .execute(&mut *conn) // Convert to &mut SqliteConnection
            .map_err(|e| anyhow!("Insert row error: {e:#?}"))
    }

    async fn insert_rows<T>(&self, obj: &Vec<&T>) -> Result<()>
    where
        T: Send + Sync + 'static,
    {
        todo!()
    }
}
