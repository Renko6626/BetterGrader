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
        // 缺考 = 本场无任何 page
        let pages: i64 = db.conn.query_row(
            "SELECT count(*) FROM page WHERE student_id=?1 AND exam_id=?2", (stu.id, exam_id), |r| r.get(0))?;
        let absent = pages == 0;
        if absent { absent_cnt += 1; }

        let mut cells = Vec::with_capacity(problems.len());
        let mut stu_total: Option<i64> = if absent { None } else { Some(0) };
        let mut all_graded = !absent;

        for (i, p) in problems.iter().enumerate() {
            if absent {
                cells.push(Cell { total: None, state: "Absent".into(), comment: None });
                all_graded = false;
                continue;
            }
            let row = db.conn.query_row(
                "SELECT total, state, comment FROM score WHERE student_id=?1 AND problem_id=?2",
                (stu.id, p.id),
                |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
            ).ok();
            let (total, state, comment) = match row {
                Some((t, s, c)) => (t, ScoreState::from_str(&s), c),
                None => (None, ScoreState::Ungraded, None),
            };
            match state {
                ScoreState::Graded => { graded += 1; }
                ScoreState::Flagged => { flagged += 1; all_graded = false; }
                ScoreState::Ungraded => { ungraded += 1; all_graded = false; }
            }
            // 临时/已判分计入总分；未判不计。只有确有分值 (Some) 才累加，
            // 避免 null-total 的 Flagged 单元用 0 拖低题目均分/学生总分。
            if matches!(state, ScoreState::Graded | ScoreState::Flagged) {
                if let Some(t) = total {
                    if let Some(st) = stu_total.as_mut() { *st += t; }
                    // 题目级：有分即计入
                    sum[i] += t;
                    cnt[i] += 1;
                }
            }
            // 不变量：Ungraded 单元 total 恒为 None（即便库中残留脏 total）。
            let cell_total = if matches!(state, ScoreState::Ungraded) { None } else { total };
            cells.push(Cell { total: cell_total, state: state.as_str().into(), comment });
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

const ABSENT_MARK: &str = "缺考";

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else { s.to_string() }
}

pub fn export_to_csv(data: &ExportData, include_comments: bool) -> String {
    let mut out = String::new();
    // 表头
    let mut header = vec!["姓名".to_string(), "考号".to_string()];
    for n in &data.problem_numbers { header.push(format!("题{n}")); }
    header.push("总分".to_string());
    if data.ranking_available { header.push("排名".to_string()); }
    if include_comments {
        for n in &data.problem_numbers { header.push(format!("题{n}评语")); }
    }
    out.push_str(&header.join(","));
    out.push('\n');

    for r in &data.rows {
        let mut f = vec![csv_field(&r.name), csv_field(r.exam_number.as_deref().unwrap_or(""))];
        for c in &r.cells {
            let cell = if r.absent { ABSENT_MARK.to_string() }
                else { match c.state.as_str() {
                    "Graded" => c.total.map(|t| t.to_string()).unwrap_or_default(),
                    "Flagged" => c.total.map(|t| format!("{t}?")).unwrap_or_else(|| "?".into()),
                    _ => String::new(), // Ungraded 留空
                }};
            f.push(csv_field(&cell));
        }
        f.push(if r.absent { ABSENT_MARK.to_string() } else { r.total.map(|t| t.to_string()).unwrap_or_default() });
        if data.ranking_available {
            f.push(r.rank.map(|x| x.to_string()).unwrap_or_default());
        }
        if include_comments {
            for c in &r.cells {
                let text = if r.absent { String::new() } else { c.comment.clone().unwrap_or_default() };
                f.push(csv_field(&text));
            }
        }
        out.push_str(&f.join(","));
        out.push('\n');
    }
    out
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

    // A1: 并列名次（标准竞赛：1,1,3）——两名 9 分并列第1，7 分跳到第3。
    #[test]
    fn tie_uses_standard_competition_ranking() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-02").unwrap();
        let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
        import_roster(&db, exam, &[
            RosterRow{name:"甲".into(), exam_number:None},
            RosterRow{name:"乙".into(), exam_number:None},
            RosterRow{name:"丙".into(), exam_number:None},
        ]).unwrap();
        let s = list_students(&db, exam).unwrap();
        for stu in &s {
            db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,1,?3,0,'labeled')",
                (exam, stu.id, format!("p{}.jpg", stu.id))).unwrap();
        }
        // 甲=9、乙=9、丙=7 → 甲/乙 并列第1，丙 第3（跳过 2）
        set_score(&db, s[0].id, p1, Some(9), None, ScoreState::Graded).unwrap();
        set_score(&db, s[1].id, p1, Some(9), None, ScoreState::Graded).unwrap();
        set_score(&db, s[2].id, p1, Some(7), None, ScoreState::Graded).unwrap();
        let d = build_export(&db, exam).unwrap();
        assert!(d.ranking_available);
        assert_eq!(d.rows.iter().find(|r| r.name=="甲").unwrap().rank, Some(1));
        assert_eq!(d.rows.iter().find(|r| r.name=="乙").unwrap().rank, Some(1));
        assert_eq!(d.rows.iter().find(|r| r.name=="丙").unwrap().rank, Some(3)); // 跳过第2
    }

    // A2: 全体缺考 → 无排名，每行 absent 且 rank=None。
    #[test]
    fn all_absent_no_ranking() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-02").unwrap();
        add_problem(&db, exam, 1, "一", 10).unwrap();
        import_roster(&db, exam, &[
            RosterRow{name:"甲".into(), exam_number:None},
            RosterRow{name:"乙".into(), exam_number:None},
        ]).unwrap();
        // 两人都不插 page → 全部缺考
        let d = build_export(&db, exam).unwrap();
        assert!(!d.ranking_available);
        assert!(d.rows.iter().all(|r| r.absent));
        assert!(d.rows.iter().all(|r| r.rank.is_none()));
        assert_eq!(d.coverage.absent, 2);
    }

    // A3: 无人作答的题目 → scored_count=0，avg/rate 均为 None（不除零）。
    #[test]
    fn scored_count_zero_gives_none_avg_rate() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-02").unwrap();
        let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
        let _p2 = add_problem(&db, exam, 2, "二", 10).unwrap(); // 无人得分
        import_roster(&db, exam, &[
            RosterRow{name:"甲".into(), exam_number:None},
        ]).unwrap();
        let s = list_students(&db, exam).unwrap();
        db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,1,?3,0,'labeled')",
            (exam, s[0].id, "p.jpg")).unwrap();
        set_score(&db, s[0].id, p1, Some(8), None, ScoreState::Graded).unwrap();
        // 题2 未判 → scored_count=0
        let d = build_export(&db, exam).unwrap();
        let ps2 = d.problem_stats.iter().find(|p| p.number==2).unwrap();
        assert_eq!(ps2.scored_count, 0);
        assert_eq!(ps2.avg, None);
        assert_eq!(ps2.rate, None);
    }

    #[test]
    fn csv_marks_states_and_omits_rank_when_incomplete() {
        let db = Db::open_in_memory().unwrap();
        let exam = scenario(&db);
        let csv = export_to_csv(&build_export(&db, exam).unwrap(), false);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "姓名,考号,题1,题2,总分"); // 有洞→无排名列
        // 甲：8,6,14
        assert!(lines.iter().any(|l| l.starts_with("甲,A1,8,6,14")));
        // 乙：存疑5标记、未判空 → "乙,A2,5?,,5"
        assert!(lines.iter().any(|l| *l == "乙,A2,5?,,5"));
        // 丙：缺考整行
        assert!(lines.iter().any(|l| *l == "丙,A3,缺考,缺考,缺考"));
    }

    #[test]
    fn csv_appends_comment_columns_when_enabled() {
        let db = Db::open_in_memory().unwrap();
        let exam = scenario(&db); // 复用现有 scenario：甲 全判/乙 存疑+未判/丙 缺考
        // 给甲题1加评语
        let s = crate::setup::list_students(&db, exam).unwrap();
        let p1 = crate::setup::list_problems(&db, exam).unwrap()[0].id;
        crate::grading::set_comment(&db, s[0].id, p1, "漂亮").unwrap();
        let data = build_export(&db, exam).unwrap();
        let no = export_to_csv(&data, false);
        let yes = export_to_csv(&data, true);
        // 不含评语时表头无评语列
        assert!(!no.lines().next().unwrap().contains("评语"));
        // 含评语时：分列在前、评语列在后
        let head = yes.lines().next().unwrap();
        assert!(head.ends_with("题1评语,题2评语") || head.contains("总分") && head.contains("题1评语"));
        assert!(head.find("总分").unwrap() < head.find("题1评语").unwrap()); // 分在前评语在后
        // 甲的题1评语出现在其行
        assert!(yes.lines().any(|l| l.starts_with("甲,") && l.contains("漂亮")));
    }

    #[test]
    fn csv_includes_rank_when_complete() {
        let db = Db::open_in_memory().unwrap();
        // 复用 ranking_appears_only_when_all_graded 的场景构造
        let exam = create_exam(&db, "E", "2026-07-02").unwrap();
        let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
        import_roster(&db, exam, &[RosterRow{name:"甲".into(),exam_number:None}]).unwrap();
        let s = list_students(&db, exam).unwrap();
        db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,1,'x.jpg',0,'labeled')", (exam, s[0].id)).unwrap();
        set_score(&db, s[0].id, p1, Some(7), None, ScoreState::Graded).unwrap();
        let csv = export_to_csv(&build_export(&db, exam).unwrap(), false);
        assert_eq!(csv.lines().next().unwrap(), "姓名,考号,题1,总分,排名");
        assert!(csv.lines().any(|l| l == "甲,,7,7,1")); // 考号 None → 空
    }

    #[test]
    fn csv_field_escapes_per_rfc4180() {
        assert_eq!(csv_field("王五"), "王五");                 // 无特殊字符不加引号
        assert_eq!(csv_field("王,五"), "\"王,五\"");            // 逗号 → 包引号
        assert_eq!(csv_field("李\"四"), "\"李\"\"四\"");        // 引号 → 双写并包引号
        assert_eq!(csv_field("a\nb"), "\"a\nb\"");             // 换行 → 包引号
    }

    #[test]
    fn csv_flagged_null_total_renders_question_mark() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-02").unwrap();
        let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
        import_roster(&db, exam, &[RosterRow{name:"甲".into(), exam_number:None}]).unwrap();
        let s = list_students(&db, exam).unwrap();
        db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,1,'x.jpg',0,'labeled')", (exam, s[0].id)).unwrap();
        // 存疑但还没给数字：total=None, state=Flagged
        set_score(&db, s[0].id, p1, None, None, ScoreState::Flagged).unwrap();
        let csv = export_to_csv(&build_export(&db, exam).unwrap(), false);
        // 甲 该题单元 = "?"（关键断言：不是空、不是 "None?"）；
        // 总分列为 "0"：build_export 对非缺考学生 stu_total 从 Some(0) 起，
        // 而 null-total 的 Flagged 单元不累加，故总分保持 0。
        assert!(csv.lines().any(|l| l == "甲,,?,0"), "got:\n{csv}");
    }
}
