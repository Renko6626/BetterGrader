use anyhow::Result;
use crate::{Db, GradingUnit, PageRef, ScoreState};

pub fn build_queue(db: &Db, exam_id: i64, problem_number: i64) -> Result<Vec<GradingUnit>> {
    // 该题 problem_id
    let (problem_id, ) : (i64,) = db.conn.query_row(
        "SELECT id FROM problem WHERE exam_id=?1 AND number=?2",
        (exam_id, problem_number), |r| Ok((r.get(0)?,)))?;

    let mut stmt = db.conn.prepare(
        "SELECT id, name FROM student WHERE exam_id=?1 ORDER BY roster_order")?;
    let students: Vec<(i64, String)> = stmt
        .query_map([exam_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;

    let mut units = Vec::new();
    for (sid, sname) in students {
        // 该 (学生,题号) 的所有 page（溢出多张），按 seq
        let mut ps = db.conn.prepare(
            "SELECT image_path FROM page WHERE student_id=?1 AND problem_number=?2 ORDER BY seq")?;
        let pages: Vec<String> = ps.query_map((sid, problem_number), |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        // 已有分数？
        let row = db.conn.query_row(
            "SELECT total, state, preset_id FROM score WHERE student_id=?1 AND problem_id=?2",
            (sid, problem_id),
            |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?)),
        ).ok();
        let (total, state, preset_id) = match row {
            Some((t, s, pid)) => (t, ScoreState::from_str(&s), pid),
            None => (None, ScoreState::Ungraded, None),
        };
        units.push(GradingUnit {
            student_id: sid, student_name: sname, problem_id, problem_number,
            pages, total, state, preset_id,
        });
    }
    Ok(units)
}

pub fn set_score(db: &Db, student_id: i64, problem_id: i64,
                 total: Option<i64>, preset_id: Option<i64>, state: ScoreState) -> Result<()> {
    db.conn.execute(
        "INSERT INTO score(student_id, problem_id, total, state, preset_id, submitted_at)
         VALUES(?1,?2,?3,?4,?5, datetime('now'))
         ON CONFLICT(student_id, problem_id)
         DO UPDATE SET total=?3, state=?4, preset_id=?5, submitted_at=datetime('now')",
        (student_id, problem_id, total, state.as_str(), preset_id),
    )?;
    Ok(())
}

pub fn student_pages(db: &Db, student_id: i64) -> Result<Vec<PageRef>> {
    let mut stmt = db.conn.prepare(
        "SELECT problem_number, image_path FROM page WHERE student_id=?1 ORDER BY seq")?;
    let rows = stmt.query_map([student_id], |r| Ok(PageRef {
        problem_number: r.get::<_, Option<i64>>(0)?.unwrap_or(0), image_path: r.get(1)?,
    }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, setup::*, ScoreState};

    fn seed(db: &Db) -> (i64, i64, i64) {
        let exam = create_exam(db, "假物理", "2026-07-02").unwrap();
        let prob = add_problem(db, exam, 1, "题一", 10).unwrap();
        import_roster(db, exam, &[
            crate::RosterRow { name: "张三".into(), exam_number: Some("A01".into()) },
            crate::RosterRow { name: "李四".into(), exam_number: Some("A02".into()) },
        ]).unwrap();
        (exam, prob, exam)
    }

    #[test]
    fn build_queue_orders_by_roster_and_defaults_ungraded() {
        let db = Db::open_in_memory().unwrap();
        let (exam, _prob, _) = seed(&db);
        let q = build_queue(&db, exam, 1).unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].student_name, "张三");
        assert_eq!(q[0].state, ScoreState::Ungraded);
        assert_eq!(q[0].total, None);   // 绝不默认 0
    }

    #[test]
    fn set_score_upserts_and_persists_state_and_preset() {
        let db = Db::open_in_memory().unwrap();
        let (exam, prob, _) = seed(&db);
        let q = build_queue(&db, exam, 1).unwrap();
        let sid = q[0].student_id;
        set_score(&db, sid, prob, Some(9), Some(42), ScoreState::Graded).unwrap();
        let q2 = build_queue(&db, exam, 1).unwrap();
        assert_eq!(q2[0].total, Some(9));
        assert_eq!(q2[0].state, ScoreState::Graded);
        assert_eq!(q2[0].preset_id, Some(42));
        // 改分：再 upsert 覆盖
        set_score(&db, sid, prob, Some(5), None, ScoreState::Flagged).unwrap();
        let q3 = build_queue(&db, exam, 1).unwrap();
        assert_eq!(q3[0].total, Some(5));
        assert_eq!(q3[0].state, ScoreState::Flagged);
        assert_eq!(q3[0].preset_id, None);   // 手动输入 preset_id=NULL
    }
}
