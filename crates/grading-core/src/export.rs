use anyhow::Result;
use crate::{Db, Cell, StudentRow, ProblemStat, Coverage, ExportData,
            persist, setup, ScoreState};

pub fn build_export(db: &Db, exam_id: i64) -> Result<ExportData> {
    let exam = persist::exam_info(db, exam_id)?;
    let problems = setup::list_problems(db, exam_id)?;
    let students = setup::list_students(db, exam_id)?;
    let problem_numbers: Vec<i64> = problems.iter().map(|p| p.number).collect();
    let problem_max: Vec<i64> = problems.iter().map(|p| p.max_score).collect();

    let (mut graded, mut flagged, mut ungraded, mut absent_cnt) = (0i64, 0i64, 0i64, 0i64);
    let mut rows: Vec<StudentRow> = Vec::new();
    // 题目级累加器
    let mut sum: Vec<i64> = vec![0; problems.len()];
    let mut cnt: Vec<i64> = vec![0; problems.len()];

    for stu in &students {
        // 缺考 = 无任何 page
        let pages: i64 = db.conn.query_row(
            "SELECT count(*) FROM page WHERE student_id=?1", [stu.id], |r| r.get(0))?;
        let absent = pages == 0;
        if absent { absent_cnt += 1; }

        let mut cells = Vec::with_capacity(problems.len());
        let mut stu_total: Option<i64> = if absent { None } else { Some(0) };
        let mut all_graded = !absent;

        for (i, p) in problems.iter().enumerate() {
            if absent {
                cells.push(Cell { total: None, state: "Absent".into() });
                all_graded = false;
                continue;
            }
            let row = db.conn.query_row(
                "SELECT total, state FROM score WHERE student_id=?1 AND problem_id=?2",
                (stu.id, p.id),
                |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?)),
            ).ok();
            let (total, state) = match row {
                Some((t, s)) => (t, ScoreState::from_str(&s)),
                None => (None, ScoreState::Ungraded),
            };
            match state {
                ScoreState::Graded => { graded += 1; }
                ScoreState::Flagged => { flagged += 1; all_graded = false; }
                ScoreState::Ungraded => { ungraded += 1; all_graded = false; }
            }
            // 临时/已判分计入总分；未判不计
            if matches!(state, ScoreState::Graded | ScoreState::Flagged) {
                if let (Some(st), Some(t)) = (stu_total.as_mut(), total) { *st += t; }
                // 题目级：有分即计入
                sum[i] += total.unwrap_or(0);
                cnt[i] += 1;
            }
            cells.push(Cell { total, state: state.as_str().into() });
        }

        rows.push(StudentRow {
            student_id: stu.id, name: stu.name.clone(), exam_number: stu.exam_number.clone(),
            absent, cells, total: stu_total, complete: all_graded, rank: None,
        });
    }

    // 排名：全部非缺考学生 complete 才出
    let non_absent: Vec<usize> = rows.iter().enumerate()
        .filter(|(_, r)| !r.absent).map(|(i, _)| i).collect();
    let ranking_available = !non_absent.is_empty()
        && non_absent.iter().all(|&i| rows[i].complete);
    if ranking_available {
        // 按 total 降序，标准竞赛名次（并列同名次，后跳位）
        let mut order: Vec<usize> = non_absent.clone();
        order.sort_by(|&a, &b| rows[b].total.cmp(&rows[a].total));
        let mut last_total: Option<i64> = None;
        let mut last_rank = 0i64;
        for (pos, &idx) in order.iter().enumerate() {
            let t = rows[idx].total;
            let rank = if t == last_total { last_rank } else { (pos as i64) + 1 };
            rows[idx].rank = Some(rank);
            last_total = t; last_rank = rank;
        }
    }

    let problem_stats = problems.iter().enumerate().map(|(i, p)| {
        let (avg, rate) = if cnt[i] > 0 {
            let a = sum[i] as f64 / cnt[i] as f64;
            (Some(a), Some(a / p.max_score as f64))
        } else { (None, None) };
        ProblemStat { number: p.number, max_score: p.max_score, avg, rate, scored_count: cnt[i] }
    }).collect();

    let coverage = Coverage {
        roster: students.len() as i64, absent: absent_cnt,
        units_total: graded + flagged + ungraded, graded, flagged, ungraded,
    };

    Ok(ExportData {
        exam, problem_numbers, problem_max, rows, problem_stats, coverage, ranking_available,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, RosterRow, setup::*, grading::*, ScoreState};

    // 造 2 题（满分 10）、3 学生：s0 全判、s1 一题存疑一题未判、s2 缺考（无 page）
    fn scenario(db: &Db) -> i64 {
        let exam = create_exam(db, "E", "2026-07-02").unwrap();
        let p1 = add_problem(db, exam, 1, "一", 10).unwrap();
        let p2 = add_problem(db, exam, 2, "二", 10).unwrap();
        import_roster(db, exam, &[
            RosterRow{name:"甲".into(), exam_number:Some("A1".into())},
            RosterRow{name:"乙".into(), exam_number:Some("A2".into())},
            RosterRow{name:"丙".into(), exam_number:Some("A3".into())},
        ]).unwrap();
        let s = list_students(db, exam).unwrap();
        // 给 s0、s1 各插 page（有 page = 到场）；s2 不插 = 缺考
        for (i, stu) in s.iter().take(2).enumerate() {
            for pn in 1..=2 {
                db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,?3,?4,?5,'labeled')",
                    (exam, stu.id, pn, format!("f{i}{pn}.jpg"), (i*2+pn as usize) as i64)).unwrap();
            }
        }
        // s0 全判：题1=8、题2=6
        set_score(db, s[0].id, p1, Some(8), None, ScoreState::Graded).unwrap();
        set_score(db, s[0].id, p2, Some(6), None, ScoreState::Graded).unwrap();
        // s1 题1 存疑=5（临时分），题2 未判
        set_score(db, s[1].id, p1, Some(5), None, ScoreState::Flagged).unwrap();
        exam
    }

    #[test]
    fn export_states_totals_and_no_ranking_when_holes() {
        let db = Db::open_in_memory().unwrap();
        let exam = scenario(&db);
        let d = build_export(&db, exam).unwrap();

        assert_eq!(d.rows.len(), 3);
        // s0 全判、total=14、complete
        let r0 = &d.rows[0];
        assert_eq!(r0.total, Some(14));
        assert!(r0.complete);
        assert_eq!(r0.cells[0].state, "Graded");
        // s1 存疑计入临时总分(5)、未判不计；不 complete
        let r1 = &d.rows[1];
        assert_eq!(r1.total, Some(5));
        assert!(!r1.complete);
        assert_eq!(r1.cells[0].state, "Flagged");
        assert_eq!(r1.cells[1].state, "Ungraded");
        assert_eq!(r1.cells[1].total, None); // 绝不填 0
        // s2 缺考
        let r2 = &d.rows[2];
        assert!(r2.absent);
        assert_eq!(r2.total, None);
        assert_eq!(r2.cells[0].state, "Absent");
        // 有洞 → 不出排名
        assert!(!d.ranking_available);
        assert!(d.rows.iter().all(|r| r.rank.is_none()));
        // 覆盖率：花名册3、缺考1、到场2×2=4 单元、已判2、存疑1、未判1
        assert_eq!(d.coverage.roster, 3);
        assert_eq!(d.coverage.absent, 1);
        assert_eq!(d.coverage.units_total, 4);
        assert_eq!(d.coverage.graded, 2);
        assert_eq!(d.coverage.flagged, 1);
        assert_eq!(d.coverage.ungraded, 1);
        // 题1得分率：非缺考有分单元 = s0(8)、s1(5) → avg 6.5、rate 0.65
        let ps1 = d.problem_stats.iter().find(|p| p.number==1).unwrap();
        assert_eq!(ps1.scored_count, 2);
        assert!((ps1.avg.unwrap()-6.5).abs()<1e-9);
        assert!((ps1.rate.unwrap()-0.65).abs()<1e-9);
    }

    #[test]
    fn ranking_appears_only_when_all_graded() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-02").unwrap();
        let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
        import_roster(&db, exam, &[
            RosterRow{name:"甲".into(), exam_number:None},
            RosterRow{name:"乙".into(), exam_number:None},
        ]).unwrap();
        let s = list_students(&db, exam).unwrap();
        for stu in &s {
            db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,1,?3,0,'labeled')",
                (exam, stu.id, format!("p{}.jpg", stu.id))).unwrap();
        }
        set_score(&db, s[0].id, p1, Some(7), None, ScoreState::Graded).unwrap();
        set_score(&db, s[1].id, p1, Some(9), None, ScoreState::Graded).unwrap();
        let d = build_export(&db, exam).unwrap();
        assert!(d.ranking_available);
        // 乙(9) 第1、甲(7) 第2
        let jiafirst = d.rows.iter().find(|r| r.name=="乙").unwrap();
        assert_eq!(jiafirst.rank, Some(1));
        assert_eq!(d.rows.iter().find(|r| r.name=="甲").unwrap().rank, Some(2));
    }
}
