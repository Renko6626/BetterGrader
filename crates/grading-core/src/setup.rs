use anyhow::Result;
use crate::{Db, Problem, Preset, Student, RosterRow};

pub fn create_exam(db: &Db, name: &str, date: &str) -> Result<i64> {
    db.conn.execute("INSERT INTO exam(name, date) VALUES(?1, ?2)", (name, date))?;
    Ok(db.conn.last_insert_rowid())
}

pub fn add_problem(db: &Db, exam_id: i64, number: i64, title: &str, max_score: i64) -> Result<i64> {
    // 全部插入在同一事务中：题目 + 三个自动档位 要么全部成功，要么全部回滚。
    // 这些函数持 `&Db`（不可变），故用 unchecked_transaction()（作用于共享 &Connection）。
    let tx = db.conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO problem(exam_id, number, title, max_score) VALUES(?1,?2,?3,?4)",
        (exam_id, number, title, max_score),
    )?;
    let pid = tx.last_insert_rowid();
    // 自动预置两个档位（内联进本事务，勿调 add_preset）：满分→9、零分→0(反引号 ` 键)。
    // 高频"零分"放左手小指最顺的 ` 上。其余档位(如部分分)用户按需自行添加。
    tx.execute("INSERT INTO score_preset(problem_id, slot, label, points) VALUES(?1,?2,?3,?4)", (pid, 9, "满分", max_score))?;
    tx.execute("INSERT INTO score_preset(problem_id, slot, label, points) VALUES(?1,?2,?3,?4)", (pid, 0, "零分", 0))?;
    tx.commit()?;
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
        "SELECT id, number, title, max_score, rubric FROM problem WHERE exam_id=?1 ORDER BY number")?;
    let rows = stmt.query_map([exam_id], |r| Ok(Problem {
        id: r.get(0)?, number: r.get(1)?, title: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        max_score: r.get(3)?, rubric: r.get(4)?,
    }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// 设置本题评分标准（Markdown）。空白视为清空（存 NULL）。
pub fn set_problem_rubric(db: &Db, problem_id: i64, rubric: &str) -> Result<()> {
    let val: Option<&str> = if rubric.trim().is_empty() { None } else { Some(rubric) };
    db.conn.execute("UPDATE problem SET rubric=?2 WHERE id=?1", (problem_id, val))?;
    Ok(())
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
    // 整份花名册在同一事务：任一行失败则全部回滚，不留半份导入。
    let tx = db.conn.unchecked_transaction()?;
    // 按 (姓名, 学号) 去重：跳过已在花名册里的同名同号者，使重复导入同一份 CSV 幂等（不翻倍）；
    // 真正的新人仍追加。同名不同号（如重名/双胞胎）视为不同人，保留。
    let mut existing: std::collections::HashSet<(String, Option<String>)> = std::collections::HashSet::new();
    {
        let mut stmt = tx.prepare("SELECT name, exam_number FROM student WHERE exam_id=?1")?;
        let it = stmt.query_map([exam_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;
        for row in it { let (n, e) = row?; existing.insert((n, e)); }
    }
    // 追加到已有花名册之后：roster_order 从当前最大值 +1 起，避免二次导入撞序号。
    let mut order: i64 = tx.query_row(
        "SELECT COALESCE(MAX(roster_order)+1, 0) FROM student WHERE exam_id=?1", [exam_id], |r| r.get(0))?;
    let mut inserted = 0usize;
    for row in rows {
        let key = (row.name.clone(), row.exam_number.clone());
        if existing.contains(&key) { continue; } // 已存在，跳过（含本份 CSV 内部重复行）
        tx.execute(
            "INSERT INTO student(exam_id, name, exam_number, roster_order) VALUES(?1,?2,?3,?4)",
            (exam_id, &row.name, &row.exam_number, order),
        )?;
        existing.insert(key); order += 1; inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

/// 解析花名册 CSV：第一列姓名、第二列学号（其余列忽略）。
/// 去 BOM、容忍 CRLF、支持带引号字段（内部逗号/双写引号）、跳过空行；
/// 若首个数据行看起来是表头（姓名/name/名字 或 第二列 学号/考号/number）则跳过。
pub fn parse_roster_csv(text: &str) -> Vec<RosterRow> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text); // 去 UTF-8 BOM
    let mut out = Vec::new();
    let mut first = true;
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() { continue; }
        let fields = parse_csv_line(line);
        let name = fields.first().map(|s| s.trim()).unwrap_or("");
        let number = fields.get(1).map(|s| s.trim()).unwrap_or("");
        if first {
            first = false;
            let n = name.to_ascii_lowercase();
            let m = number.to_ascii_lowercase();
            if matches!(n.as_str(), "姓名" | "name" | "名字")
                || matches!(m.as_str(), "学号" | "考号" | "number" | "exam_number") {
                continue; // 跳过表头行
            }
        }
        if name.is_empty() { continue; }
        out.push(RosterRow {
            name: name.to_string(),
            exam_number: if number.is_empty() { None } else { Some(number.to_string()) },
        });
    }
    out
}

// 单行 CSV 字段拆分：支持 "..." 引号字段（内部 "" 转义为一个引号、可含逗号）。
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') { cur.push('"'); chars.next(); } // 转义引号
                else { in_quotes = false; }
            }
            '"' if cur.is_empty() => in_quotes = true, // 仅字段起始的引号才开引号态
            '"' => cur.push('"'),                      // 字段中间的裸引号按字面量，不吞掉后面的逗号
            ',' if !in_quotes => { fields.push(std::mem::take(&mut cur)); }
            _ => cur.push(c),
        }
    }
    fields.push(cur);
    fields
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
    fn add_problem_auto_seeds_two_presets() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let p = add_problem(&db, exam, 3, "力学大题", 20).unwrap();
        let presets = list_presets(&db, p).unwrap();
        // 满分@9=max=20、零分@0(反引号)=0 两个自动档位（不再有冗余的"空白"）
        assert_eq!(presets.len(), 2);
        assert!(presets.iter().any(|x| x.label == "满分" && x.points == 20 && x.slot == 9));
        assert!(presets.iter().any(|x| x.label == "零分" && x.points == 0 && x.slot == 0));
    }

    #[test]
    fn add_custom_preset_and_list_problems() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let p = add_problem(&db, exam, 3, "力学大题", 20).unwrap();
        add_preset(&db, p, 2, "前两问", 9).unwrap();
        assert_eq!(list_presets(&db, p).unwrap().len(), 3); // 满分+零分 两个自动 + 1 自定义
        let probs = list_problems(&db, exam).unwrap();
        assert_eq!(probs, vec![Problem { id: p, number: 3, title: "力学大题".into(), max_score: 20, rubric: None }]);
    }

    #[test]
    fn set_and_clear_problem_rubric() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let p = add_problem(&db, exam, 1, "题1", 10).unwrap();
        assert_eq!(list_problems(&db, exam).unwrap()[0].rubric, None); // 建题默认无标准
        set_problem_rubric(&db, p, "- 前两问各 3 分\n- 末问 4 分").unwrap();
        assert_eq!(list_problems(&db, exam).unwrap()[0].rubric.as_deref(), Some("- 前两问各 3 分\n- 末问 4 分"));
        set_problem_rubric(&db, p, "   ").unwrap(); // 空白 = 清空
        assert_eq!(list_problems(&db, exam).unwrap()[0].rubric, None);
    }

    #[test]
    fn parse_roster_csv_basics() {
        // 去 BOM + 跳表头 + 空行 + 缺学号 + 带引号内含逗号
        let csv = "\u{feff}姓名,学号\n张三,A01\n李四,\n\n\"王, 五\",A03\n";
        let rows = parse_roster_csv(csv);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], RosterRow { name: "张三".into(), exam_number: Some("A01".into()) });
        assert_eq!(rows[1], RosterRow { name: "李四".into(), exam_number: None });
        assert_eq!(rows[2], RosterRow { name: "王, 五".into(), exam_number: Some("A03".into()) });
    }

    #[test]
    fn parse_roster_csv_bare_quote_keeps_delimiter() {
        // 字段中间的裸引号不得吞掉后面的逗号（否则学号会并进姓名丢失）
        let rows = parse_roster_csv("李四\",A03\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].exam_number.as_deref(), Some("A03")); // 逗号仍分隔，学号保住
    }

    #[test]
    fn parse_roster_csv_no_header_keeps_first_row() {
        let rows = parse_roster_csv("陈一,B01\n周二,B02\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "陈一");
    }

    #[test]
    fn import_roster_appends_after_existing() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        import_roster(&db, exam, &[RosterRow { name: "甲".into(), exam_number: None }]).unwrap();
        import_roster(&db, exam, &[RosterRow { name: "乙".into(), exam_number: None }]).unwrap();
        let students = list_students(&db, exam).unwrap(); // 按 roster_order
        assert_eq!(students.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["甲", "乙"]);
        assert_eq!(students[1].roster_order, Some(1)); // 追加而非撞 0
    }

    #[test]
    fn import_roster_dedups_on_reimport() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "假物理", "2026-07-02").unwrap();
        let csv = &[
            RosterRow { name: "张三".into(), exam_number: Some("A01".into()) },
            RosterRow { name: "李四".into(), exam_number: None },
        ];
        assert_eq!(import_roster(&db, exam, csv).unwrap(), 2);
        assert_eq!(import_roster(&db, exam, csv).unwrap(), 0); // 重导同一份：零新增
        // 混入一个新人 + 两个老人：只加新人
        let mixed = &[
            RosterRow { name: "张三".into(), exam_number: Some("A01".into()) },
            RosterRow { name: "王五".into(), exam_number: Some("A03".into()) },
        ];
        assert_eq!(import_roster(&db, exam, mixed).unwrap(), 1);
        assert_eq!(list_students(&db, exam).unwrap().len(), 3); // 张三、李四、王五
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
