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
        migrate(&conn)?;
        Ok(Db { conn })
    }
}

// 既有库补 score.comment 列（新库已由 schema 建好，pragma 检测避免重复 ALTER）
pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    let has: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('score') WHERE name='comment'",
        [],
        |r| r.get(0),
    )?;
    if has == 0 {
        conn.execute("ALTER TABLE score ADD COLUMN comment TEXT", [])?;
    }
    Ok(())
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

    #[test]
    fn score_has_comment_column_and_migration_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        // 列存在
        let has: i64 = db.conn.query_row(
            "SELECT count(*) FROM pragma_table_info('score') WHERE name='comment'", [], |r| r.get(0)).unwrap();
        assert_eq!(has, 1);
        // 再跑一次迁移不报错（幂等）
        super::migrate(&db.conn).unwrap();
        let has2: i64 = db.conn.query_row(
            "SELECT count(*) FROM pragma_table_info('score') WHERE name='comment'", [], |r| r.get(0)).unwrap();
        assert_eq!(has2, 1);
    }
}
