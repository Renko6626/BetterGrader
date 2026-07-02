use anyhow::Result;
use crate::{Db, Problem, Preset, Student, RosterRow};

pub fn create_exam(db: &Db, name: &str, date: &str) -> Result<i64> {
    db.conn.execute("INSERT INTO exam(name, date) VALUES(?1, ?2)", (name, date))?;
    Ok(db.conn.last_insert_rowid())
}

pub fn add_problem(db: &Db, exam_id: i64, number: i64, title: &str, max_score: i64) -> Result<i64> {
    db.conn.execute(
        "INSERT INTO problem(exam_id, number, title, max_score) VALUES(?1,?2,?3,?4)",
        (exam_id, number, title, max_score),
    )?;
    let pid = db.conn.last_insert_rowid();
    // 自动预置三个档位：满分/零分/空白
    add_preset(db, pid, 7, "满分", max_score)?;
    add_preset(db, pid, 8, "零分", 0)?;
    add_preset(db, pid, 9, "空白", 0)?;
    Ok(pid)
}

pub fn add_preset(db: &Db, problem_id: i64, slot: i64, label: &str, points: i64) -> Result<i64> {
    db.conn.execute(
        "INSERT INTO score_preset(problem_id, slot, label, points) VALUES(?1,?2,?3,?4)",
        (problem_id, slot, label, points),
    )?;
    Ok(db.conn.last_insert_rowid())
}

pub fn list_problems(db: &Db, exam_id: i64) -> Result<Vec<Problem>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, number, title, max_score FROM problem WHERE exam_id=?1 ORDER BY number")?;
    let rows = stmt.query_map([exam_id], |r| Ok(Problem {
        id: r.get(0)?, number: r.get(1)?, title: r.get::<_, Option<String>>(2)?.unwrap_or_default(), max_score: r.get(3)?,
    }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn list_presets(db: &Db, problem_id: i64) -> Result<Vec<Preset>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, slot, label, points FROM score_preset WHERE problem_id=?1 ORDER BY slot")?;
    let rows = stmt.query_map([problem_id], |r| Ok(Preset {
        id: r.get(0)?, slot: r.get(1)?, label: r.get(2)?, points: r.get(3)?,
    }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn import_roster(db: &Db, exam_id: i64, rows: &[RosterRow]) -> Result<usize> {
    for (i, row) in rows.iter().enumerate() {
        db.conn.execute(
            "INSERT INTO student(exam_id, name, exam_number, roster_order) VALUES(?1,?2,?3,?4)",
            (exam_id, &row.name, &row.exam_number, i as i64),
        )?;
    }
    Ok(rows.len())
}

pub fn list_students(db: &Db, exam_id: i64) -> Result<Vec<Student>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, name, exam_number, roster_order FROM student WHERE exam_id=?1 ORDER BY roster_order")?;
    let rows = stmt.query_map([exam_id], |r| Ok(Student {
        id: r.get(0)?, name: r.get(1)?, exam_number: r.get(2)?, roster_order: r.get(3)?,
    }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    #[test]
    fn add_problem_auto_seeds_three_presets() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let p = add_problem(&db, exam, 3, "力学大题", 20).unwrap();
        let presets = list_presets(&db, p).unwrap();
        // 满分=max=20、零分=0、空白=0 三个自动档位
        assert_eq!(presets.len(), 3);
        assert!(presets.iter().any(|x| x.label == "满分" && x.points == 20));
        assert!(presets.iter().any(|x| x.label == "零分" && x.points == 0));
        assert!(presets.iter().any(|x| x.label == "空白" && x.points == 0));
    }

    #[test]
    fn add_custom_preset_and_list_problems() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let p = add_problem(&db, exam, 3, "力学大题", 20).unwrap();
        add_preset(&db, p, 2, "前两问", 9).unwrap();
        assert_eq!(list_presets(&db, p).unwrap().len(), 4);
        let probs = list_problems(&db, exam).unwrap();
        assert_eq!(probs, vec![Problem { id: p, number: 3, title: "力学大题".into(), max_score: 20 }]);
    }

    #[test]
    fn import_roster_and_list_students() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let rows = vec![
            RosterRow { name: "张三".into(), exam_number: Some("A01".into()) },
            RosterRow { name: "李四".into(), exam_number: Some("A02".into()) },
        ];
        assert_eq!(import_roster(&db, exam, &rows).unwrap(), 2);
        let students = list_students(&db, exam).unwrap();
        assert_eq!(students.len(), 2);
        assert_eq!(students[0].name, "张三");
        assert_eq!(students[0].roster_order, Some(0));
    }
}
