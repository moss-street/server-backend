use anyhow::{anyhow, Result};
use async_trait::async_trait;

use diesel::connection::LoadConnection;
use diesel::expression::SqlLiteral;
use diesel::query_builder::{AsQuery, InsertStatement, QueryFragment, QueryId};
use diesel::query_dsl::methods::{FilterDsl, LoadQuery, LockingDsl};
use diesel::query_dsl::{methods::BoxedDsl, QueryDsl};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::{Bool, Text};
use diesel::sqlite::SqliteConnection;
use diesel::{query_dsl::methods::ExecuteDsl, sqlite::Sqlite, Table};
use diesel::{Insertable, RunQueryDsl, Queryable};
use diesel::dsl::sql;

#[async_trait]
pub trait DatabaseImpl {
    async fn query_row<'a, T, U>(&self, table: T, fields: Vec<(&str, &str)>) -> Result<Vec<U>>
    where
    T: Table + FilterDsl<SqlLiteral<Bool>> + Send + LoadConnection + 'static,
    U: Queryable<T::SqlType, Sqlite> + Send + Sync + 'static,
    <T as FilterDsl<SqlLiteral<diesel::sql_types::Bool>>>::Output: Table + diesel::RunQueryDsl<T>+ LoadQuery<'a, T, U>;

    async fn query_rows<T>(&self, fields: Vec<(&str, &str)>) -> Result<Vec<T>>
    where
        T: Send + Sync + 'static;

    fn insert_row<T, U>(&self, table: T, obj: &U) -> Result<usize>
    where
        T: Table + Send + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>;

    async fn insert_rows<T, 'a, 'b>(&self, _obj: &'a [&'b T]) -> Result<()>
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
    async fn query_row<'a, T, U>(&self, table: T, fields: Vec<(&str, &str)>) -> Result<Vec<U>>
    where
    T: Table + FilterDsl<SqlLiteral<Bool>> + Send + LoadConnection + 'static,
    U: Queryable<T::SqlType, Sqlite> + Send + Sync + 'static,
    <T as FilterDsl<SqlLiteral<diesel::sql_types::Bool>>>::Output: Table + diesel::RunQueryDsl<T>+ LoadQuery<'a, T, U>
    {
        let Some(mut conn) = self.connection_pool.try_get() else {
            return Err(anyhow!("No available connection in connection pool!"));
        };
        
        // Start with an empty filter string
        let mut filter_string = String::from("");

        // Dynamically build the filter string with AND conditions
        for (col, val) in fields {
            // Add each condition as "column = value"
            let condition = format!("{} = ?", col);
            filter_string.push_str(" AND ");
            filter_string.push_str(&condition);
        }

        // Create the SQL filter expression
        let query = table.filter(sql::<Bool>(&filter_string));


        let results = query  // bind the first parameter as a Text type
            .load::<U>(&mut *conn)
            .map_err(|e| anyhow!("Query row error: {e:#?}"))?;

        Ok(results)
    
    }
    async fn query_rows<T>(&self, _fields: Vec<(&str, &str)>) -> Result<Vec<T>>
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

    async fn insert_rows<T, 'a, 'b>(&self, _obj: &'a [&'b T]) -> Result<()>
    where
        T: Send + Sync + 'static,
    {
        todo!()
    }
}
