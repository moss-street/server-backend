use anyhow::{anyhow, Result};
use std::future::Future;

use diesel::dsl::sql;
use diesel::expression::SqlLiteral;
use diesel::query_builder::{InsertStatement, QueryFragment, QueryId};
use diesel::query_dsl::methods::{FilterDsl, LoadQuery};
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::Bool;
use diesel::sqlite::SqliteConnection;
use diesel::{query_dsl::methods::ExecuteDsl, sqlite::Sqlite, Table};
use diesel::{Insertable, QueryDsl, Queryable, RunQueryDsl};

pub trait DatabaseImpl {
    fn query_row<'a, T, U>(
        &self,
        table: T,
        fields: Vec<(&str, &str)>,
    ) -> impl Future<Output = Result<Vec<U>>>
    where
        T: Table + QueryDsl + 'static,
        T::Query: FilterDsl<SqlLiteral<Bool>>,
        <T::Query as FilterDsl<SqlLiteral<Bool>>>::Output: LoadQuery<'a, SqliteConnection, U>,
        U: Queryable<T::SqlType, Sqlite> + Send + Sync + 'static;

    fn query_rows<T>(&self, fields: Vec<(&str, &str)>) -> impl Future<Output = Result<Vec<T>>>
    where
        T: Send + Sync + 'static;

    fn insert_row<T, U>(&self, table: T, obj: &U) -> Result<usize>
    where
        T: Table + Send + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>;

    fn insert_rows<T>(&self, _obj: &[&T]) -> Result<()>
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

impl DatabaseImpl for DBManager {
    async fn query_row<'a, T, U>(&self, table: T, fields: Vec<(&str, &str)>) -> Result<Vec<U>>
    where
        T: Table + QueryDsl + 'static,
        T::Query: FilterDsl<SqlLiteral<Bool>>,
        <T::Query as FilterDsl<SqlLiteral<Bool>>>::Output: LoadQuery<'a, SqliteConnection, U>,
        U: Queryable<T::SqlType, Sqlite> + Send + Sync + 'static,
    {
        let Some(mut conn) = self.connection_pool.try_get() else {
            return Err(anyhow!("No available connection in connection pool!"));
        };

        // Start with an empty filter string
        let mut filter_string = String::from("");

        // Dynamically build the filter string with AND conditions
        for (col, _val) in fields {
            // Add each condition as "column = value"
            let condition = format!("{} = ?", col);
            filter_string.push_str(" AND ");
            filter_string.push_str(&condition);
        }

        // Create the SQL filter expression
        let query = table.filter(sql::<Bool>(&filter_string));

        let results = query // bind the first parameter as a Text type
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

    fn insert_rows<T>(&self, _obj: &[&T]) -> Result<()>
    where
        T: Send + Sync + 'static,
    {
        todo!()
    }
}
