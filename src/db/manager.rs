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
    fn query_rows<'a, T, U>(
        &self,
        table: T,
        fields: Vec<(&str, &str)>,
    ) -> impl Future<Output = Result<Vec<U>>>
    where
        T: Table + QueryDsl + 'static,
        T::Query: FilterDsl<SqlLiteral<Bool>>,
        <T::Query as FilterDsl<SqlLiteral<Bool>>>::Output: LoadQuery<'a, SqliteConnection, U>,
        U: Queryable<T::SqlType, Sqlite> + Send + Sync + 'static;

    fn insert_row<T, U>(&self, table: T, obj: &U) -> Result<usize>
    where
        T: Table + Send + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>;

    fn insert_rows<T, U>(&self, table: T, objs: Vec<&U>) -> Result<usize>
    where
        T: Table + Send + Clone + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>;
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
    async fn query_rows<'a, T, U>(&self, table: T, fields: Vec<(&str, &str)>) -> Result<Vec<U>>
    where
        T: Table + QueryDsl + 'static,
        T::Query: FilterDsl<SqlLiteral<Bool>>,
        <T::Query as FilterDsl<SqlLiteral<Bool>>>::Output: LoadQuery<'a, SqliteConnection, U>,
        U: Queryable<T::SqlType, Sqlite> + Send + Sync + 'static,
    {
        let Some(mut conn) = self.connection_pool.try_get() else {
            return Err(anyhow!("No available connection in connection pool!"));
        };

        let filter_string = fields
            .iter()
            .map(|(col, val)| format!(" {} = '{}' ", col, val))
            .collect::<Vec<_>>()
            .join("AND");

        // Create the SQL filter expression
        let query = table.filter(sql::<Bool>(&filter_string));

        let results = query // bind the first parameter as a Text type
            .load::<U>(&mut *conn)
            .map_err(|e| anyhow!("Query row error: {e:#?}"))?;

        Ok(results)
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

    fn insert_rows<T, U>(&self, table: T, objs: Vec<&U>) -> Result<usize>
    where
        T: Table + Send + Clone + 'static,
        U: Insertable<T> + Clone + Send,
        <U as Insertable<T>>::Values: QueryFragment<Sqlite> + QueryId + Send,
        InsertStatement<T, <U as Insertable<T>>::Values>: ExecuteDsl<SqliteConnection>,
    {
        // A janky way of inserting bulk rows iteratively calling insert_row
        // Ideally this would be it's own function which can do a bulk insert but rust
        // is hard.
        // TODO: when someone with skill can, make this bulk insert objs with one query
        Ok(objs
            .iter()
            .map(|o| {
                let table = table.clone();
                self.insert_row(table, *o).ok()
            })
            .map(|r| r.unwrap_or(0))
            .sum())
    }
}
