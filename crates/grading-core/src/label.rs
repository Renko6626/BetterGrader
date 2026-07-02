use anyhow::Result;
use crate::{Db, PageRow};

pub fn list_pages(db: &Db, exam_id: i64) -> Result<Vec<PageRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, seq, image_path, student_id, problem_number, status
         FROM page WHERE exam_id=?1 ORDER BY seq")?;
    let rows = stmt.query_map([exam_id], |r| Ok(PageRow {
        id: r.get(0)?, seq: r.get::<_, Option<i64>>(1)?.unwrap_or(0), image_path: r.get(2)?,
        student_id: r.get(3)?, problem_number: r.get(4)?, status: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
    }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn set_page_label(db: &Db, page_id: i64, student_id: Option<i64>, problem_number: Option<i64>) -> Result<()> {
    let status = if student_id.is_some() { "labeled" } else { "ingested" };
    db.conn.execute(
        "UPDATE page SET student_id=?2, problem_number=?3, status=?4 WHERE id=?1",
        (page_id, student_id, problem_number, status))?;
    Ok(())
}

pub fn add_student(db: &Db, exam_id: i64, name: &str, exam_number: Option<&str>) -> Result<i64> {
    let next: i64 = db.conn.query_row(
        "SELECT COALESCE(MAX(roster_order)+1, 0) FROM student WHERE exam_id=?1", [exam_id], |r| r.get(0))?;
    db.conn.execute(
        "INSERT INTO student(exam_id, name, exam_number, roster_order) VALUES(?1,?2,?3,?4)",
        (exam_id, name, exam_number, next))?;
    Ok(db.conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, setup::create_exam, ingest::add_ingested_page};

    #[test]
    fn list_set_label_and_add_student() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-03").unwrap();
        let p0 = add_ingested_page(&db, exam, "a.jpg", 0).unwrap();
        add_ingested_page(&db, exam, "b.jpg", 1).unwrap();

        let pages = list_pages(&db, exam).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].seq, 0);
        assert_eq!(pages[0].status, "ingested");
        assert_eq!(pages[0].student_id, None);

        let sid = add_student(&db, exam, "张三", Some("A1")).unwrap();
        // 姓名页：problem_number=0
        set_page_label(&db, p0, Some(sid), Some(0)).unwrap();
        let pages2 = list_pages(&db, exam).unwrap();
        assert_eq!(pages2[0].student_id, Some(sid));
        assert_eq!(pages2[0].problem_number, Some(0));
        assert_eq!(pages2[0].status, "labeled");
        // 撤销标注 → 回 ingested
        set_page_label(&db, p0, None, None).unwrap();
        assert_eq!(list_pages(&db, exam).unwrap()[0].status, "ingested");
    }
}
