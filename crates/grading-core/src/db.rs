use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Db {
    pub conn: Connection,
}

const SCHEMA: &str = include_str!("schema.sql");

impl Db {
    pub fn open_in_memory() -> Result<Db> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }
    fn init(conn: Connection) -> Result<Db> {
        conn.execute_batch(&format!("PRAGMA foreign_keys=ON;\n{SCHEMA}"))?;
        Ok(Db { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opens_in_memory_and_creates_tables() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN
             ('exam','problem','score_preset','student','page','score','rubric_step','score_item')",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 8);
    }
}
