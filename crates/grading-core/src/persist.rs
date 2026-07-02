use anyhow::Result;
use rusqlite::OptionalExtension;
use crate::{Db, ExamInfo, setup};

/// 每个 exam.db 只含一场考试：已有则返回其 id，否则新建。
pub fn ensure_exam(db: &Db, name: &str, date: &str) -> Result<i64> {
    let existing: Option<i64> = db.conn
        .query_row("SELECT id FROM exam ORDER BY id LIMIT 1", [], |r| r.get(0))
        .optional()?;
    match existing {
        Some(id) => Ok(id),
        None => setup::create_exam(db, name, date),
    }
}

pub fn exam_info(db: &Db, exam_id: i64) -> Result<ExamInfo> {
    let info = db.conn.query_row(
        "SELECT id, name, date FROM exam WHERE id=?1",
        [exam_id],
        |r| Ok(ExamInfo { id: r.get(0)?, name: r.get(1)?, date: r.get::<_, Option<String>>(2)?.unwrap_or_default() }),
    )?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    #[test]
    fn ensure_exam_creates_once_then_returns_same() {
        let db = Db::open_in_memory().unwrap();
        let a = ensure_exam(&db, "某场奥赛", "2026-07-02").unwrap();
        let b = ensure_exam(&db, "改个名也无所谓", "2026-07-03").unwrap();
        assert_eq!(a, b); // 单库单场：第二次返回已有 id，不新建
        let info = exam_info(&db, a).unwrap();
        assert_eq!(info.name, "某场奥赛");
        assert_eq!(info.date, "2026-07-02");
    }

    #[test]
    fn exam_info_null_date_returns_empty_string() {
        let db = Db::open_in_memory().unwrap();
        // 直接插入 date 为 NULL 的一行
        db.conn.execute("INSERT INTO exam(name, date) VALUES('X', NULL)", []).unwrap();
        // 单库单场：ensure_exam 返回已有（NULL date）行的 id
        let id = ensure_exam(&db, "无所谓", "2026-07-02").unwrap();
        let info = exam_info(&db, id).unwrap();
        assert_eq!(info.name, "X");
        assert_eq!(info.date, ""); // NULL → 空串
    }
}
