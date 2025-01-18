use crate::db::manager::TableImpl;
use rusqlite::Row;

#[allow(unused)]
pub struct Stock {
    id: i32,
    name: String,
}

impl TableImpl for Stock {
    fn create_table_query() -> String {
        String::from(
            r#"CREATE TABLE IF NOT EXISTS Stock (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);"#,
        )
    }

    fn generate_db_load_query(&self) -> String {
        todo!()
    }

    fn generate_db_lookup_query(_id: i32) -> String {
        todo!()
    }

    fn deserialize_query_result(_result: &Row) -> Result<Self, rusqlite::Error> {
        todo!()
    }
}
