use anyhow::Result;
use crate::{Db, RosterRow, setup::*};

pub fn seed_fake_exam(db: &Db) -> Result<i64> {
    let exam = create_exam(db, "假奥赛物理", "2026-07-02")?;
    // 3 道题 + 各一个语义化自定义档位
    let p1 = add_problem(db, exam, 1, "运动学", 10)?;
    add_preset(db, p1, 1, "只列式", 4)?;
    let p2 = add_problem(db, exam, 2, "动力学", 20)?;
    add_preset(db, p2, 1, "只建系受力", 6)?;
    add_preset(db, p2, 2, "建系+列方程", 11)?;
    let p3 = add_problem(db, exam, 3, "能量", 20)?;
    add_preset(db, p3, 1, "前两问", 12)?;

    let names = ["张三", "李四", "王五", "赵六", "钱七"];
    let rows: Vec<RosterRow> = names.iter().enumerate()
        .map(|(i, n)| RosterRow { name: (*n).into(), exam_number: Some(format!("A{:02}", i + 1)) })
        .collect();
    import_roster(db, exam, &rows)?;
    let students = list_students(db, exam)?;

    // 每个学生每题一张 page；第 3 个学生第 2 题溢出两张
    let mut seq = 0i64;
    for (idx, s) in students.iter().enumerate() {
        for prob_no in 1..=3 {
            insert_page(db, exam, s.id, prob_no, &mut seq)?;
            if idx == 2 && prob_no == 2 {
                insert_page(db, exam, s.id, prob_no, &mut seq)?; // 溢出续页
            }
        }
    }
    Ok(exam)
}

fn insert_page(db: &Db, exam_id: i64, student_id: i64, prob_no: i64, seq: &mut i64) -> Result<()> {
    let path = format!("fake://exam{exam_id}/stu{student_id}/q{prob_no}/{seq}.jpg");
    db.conn.execute(
        "INSERT INTO page(exam_id, student_id, problem_number, image_path, seq, status)
         VALUES(?1,?2,?3,?4,?5,'labeled')",
        (exam_id, student_id, prob_no, path, *seq),
    )?;
    *seq += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grading::{build_queue, student_pages};

    #[test]
    fn seed_produces_thick_data_for_axis2() {
        let db = Db::open_in_memory().unwrap();
        let exam = seed_fake_exam(&db).unwrap();
        assert_eq!(list_problems(&db, exam).unwrap().len(), 3);
        assert_eq!(list_students(&db, exam).unwrap().len(), 5);
        // 队列覆盖全部 5 个学生
        let q = build_queue(&db, exam, 1).unwrap();
        assert_eq!(q.len(), 5);
        // 第 1 个学生跨题都有页（轴2 速览需要）
        let s1 = q[0].student_id;
        let pages = student_pages(&db, s1).unwrap();
        assert!(pages.iter().any(|p| p.problem_number == 1));
        assert!(pages.iter().any(|p| p.problem_number == 2));
        assert!(pages.iter().any(|p| p.problem_number == 3));
        // 溢出：某个 (学生,题号) 挂两张
        let overflow = build_queue(&db, exam, 2).unwrap().into_iter()
            .any(|u| u.pages.len() == 2);
        assert!(overflow);
    }
}
