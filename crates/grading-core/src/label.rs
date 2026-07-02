use anyhow::Result;
use crate::{Db, PageRow, StackRow, LabelSummary, setup};

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

pub fn labeling_summary(db: &Db, exam_id: i64) -> Result<LabelSummary> {
    let problem_count = setup::list_problems(db, exam_id)?.len() as i64;
    let students = setup::list_students(db, exam_id)?;

    let mut stacks = Vec::new();
    let mut absent_students = Vec::new();
    for stu in &students {
        let total_pages: i64 = db.conn.query_row(
            "SELECT count(*) FROM page WHERE student_id=?1", [stu.id], |r| r.get(0))?;
        if total_pages == 0 { absent_students.push(stu.clone()); continue; }
        let answer_pages: i64 = db.conn.query_row(
            "SELECT count(*) FROM page WHERE student_id=?1 AND problem_number >= 1", [stu.id], |r| r.get(0))?;
        stacks.push(StackRow {
            student_id: stu.id, student_name: stu.name.clone(),
            answer_pages, problem_count, count_ok: answer_pages == problem_count,
        });
    }
    let unlabeled_pages: i64 = db.conn.query_row(
        "SELECT count(*) FROM page WHERE exam_id=?1 AND student_id IS NULL", [exam_id], |r| r.get(0))?;

    Ok(LabelSummary { stacks, absent_students, unlabeled_pages })
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

    #[test]
    fn summary_counts_pages_and_flags_mismatch_and_absent() {
        use crate::setup::{add_problem, import_roster};
        use crate::RosterRow;
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-03").unwrap();
        add_problem(&db, exam, 1, "一", 10).unwrap();
        add_problem(&db, exam, 2, "二", 10).unwrap();     // N=2
        import_roster(&db, exam, &[
            RosterRow{name:"甲".into(), exam_number:None},
            RosterRow{name:"乙".into(), exam_number:None},
            RosterRow{name:"丙".into(), exam_number:None},  // 丙 缺考
            RosterRow{name:"丁".into(), exam_number:None},  // 丁 只有姓名页
        ]).unwrap();
        let s: Vec<i64> = crate::setup::list_students(&db, exam).unwrap().iter().map(|x| x.id).collect();
        // 甲：姓名页(0) + 题1 + 题2 = 答题页 2 == N ✓
        for (seq, pn) in [(0,0),(1,1),(2,2)] { let id=add_ingested_page(&db,exam,&format!("j{seq}.jpg"),seq).unwrap(); set_page_label(&db,id,Some(s[0]),Some(pn)).unwrap(); }
        // 乙：姓名页 + 只有题1 = 答题页 1 != 2 ✗（§7.0 报警）
        for (seq, pn) in [(3,0),(4,1)] { let id=add_ingested_page(&db,exam,&format!("y{seq}.jpg"),seq).unwrap(); set_page_label(&db,id,Some(s[1]),Some(pn)).unwrap(); }
        // 丁：仅姓名页(0)，无答题页 → 答题页 0 != 2 ✗，但有 page → 进 stacks，不缺考
        { let id=add_ingested_page(&db,exam,"d0.jpg",8).unwrap(); set_page_label(&db,id,Some(s[3]),Some(0)).unwrap(); }
        // 一张未标注
        add_ingested_page(&db, exam, "x.jpg", 9).unwrap();

        let sum = labeling_summary(&db, exam).unwrap();
        let jia = sum.stacks.iter().find(|r| r.student_id==s[0]).unwrap();
        assert_eq!((jia.answer_pages, jia.problem_count, jia.count_ok), (2, 2, true));
        let yi = sum.stacks.iter().find(|r| r.student_id==s[1]).unwrap();
        assert_eq!((yi.answer_pages, yi.count_ok), (1, false));   // 页数≠N
        assert_eq!(yi.problem_count, 2);                          // N=2
        // 丁：只有姓名页 → 进 stacks，answer_pages=0，页数不符，且不算缺考
        let ding = sum.stacks.iter().find(|r| r.student_id==s[3]).unwrap();
        assert_eq!((ding.answer_pages, ding.count_ok), (0, false));
        assert!(sum.absent_students.iter().all(|st| st.name != "丁"));
        assert!(sum.stacks.iter().all(|r| r.student_id != s[2])); // 丙无 page → 不在 stacks
        assert_eq!(sum.absent_students.len(), 1);                 // 仅丙 缺考
        assert_eq!(sum.absent_students[0].name, "丙");
        assert_eq!(sum.unlabeled_pages, 1);
    }
}
