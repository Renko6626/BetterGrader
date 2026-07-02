# 阅卷辅助器 · 持久化 + 导出 M3 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让判分结果真正落地——把内存库换成每场一个自包含 `exam.db` 目录，并一键导出可交差的成绩表（CSV 真文件 + 可打印 PDF 报表），三态清晰、异常绝不静默填 0、带洞不出排名。

**Architecture:** `grading-core` 保持纯逻辑（新增 exam 句柄辅助 + 导出计算/CSV 序列化，`cargo test` 全覆盖）。`src-tauri` 把 `AppState` 从"永远开着的内存库"改为"当前打开的考试"（`Mutex<Option<OpenExam>>`），用 tauri-plugin-dialog 选目录开/建考试。导出：CSV 由 Rust 写真文件（路径来自保存对话框），PDF 走"渲染可打印 HTML 报表 → webview 打印/另存 PDF"，绕开 Rust PDF 库的中文字体嵌入坑。

**Tech Stack:** 沿用 M0/M1（Tauri v2 · rusqlite bundled · Vue3+TS+Vite · Vitest · naive-ui）+ 新增 `tauri-plugin-dialog`（Rust）与 `@tauri-apps/plugin-dialog`（JS）。

## Global Constraints

以下为全项目约束，每个任务隐含包含（数值/措辞照抄自 spec §4/§9）：

- **一场考试 = 一个自包含目录** `/某场考试/{images/, exam.db}`，可整体拎走归档；数据不出本地。
- **每个 `exam.db` 只含一场考试**（`exam` 表恰一行）。
- **判分单元三态** `Ungraded/Flagged/Graded` + 学生层 `缺考`（花名册有、无 page 绑定）。**导出绝不把 未判/存疑 静默当 0**。
- **存疑临时分计入临时总分**（它是真敲的数）；**未判不贡献分**（留空）。
- **排名只在"纳入学生全部已判"时才计算**，否则**整列不出/标"不完整"**——绝不用带洞数据排出看着像真的名次。
- **缺考学生**整行标 `缺考`，不参与排名与题目级统计。
- **题目级得分率** = 该题（非缺考、已给分单元）平均分 / 满分。
- CSV 列：`姓名, 考号, 题1..题N, 总分, [排名]`；`未判` 格留空、`存疑` 格标记、`缺考` 行整标。
- **AI 永不写 `score.total`**（架构约束，本片无 AI）。
- UI = naive-ui + 暗色主题（已在 App.vue 全局包裹）。
- **PDF 生成 = 可打印 HTML + webview 打印**（不引 Rust PDF 库；决策见架构，评审可否决）。

---

## File Structure

```
crates/grading-core/src/
  persist.rs      # 【新】ensure_exam / exam_info（每库单场）
  export.rs       # 【新】build_export（导出数据模型+计算）+ export_to_csv
  models.rs       # 追加 ExamInfo / 导出相关 struct
  lib.rs          # 追加 pub mod persist; pub mod export;
src-tauri/
  capabilities/default.json   # 【新】授予 main 窗口 dialog 插件权限
  src/commands.rs             # AppState 改 Option<OpenExam>；新增 exam 生命周期 + 导出命令
  src/main.rs                 # 注册新命令 + dialog 插件；不再 open_in_memory
  Cargo.toml                  # + tauri-plugin-dialog
src/
  api.ts          # 追加 newExam/openExam/seedDemoExam/currentExam/exportSummary/saveCsv 封装
  types.ts        # 追加 ExamInfo / ExportData 等 TS 类型
  views/
    SetupView.vue # 改：开/建/演示考试入口（dialog 选目录），展示当前考试
    ExportView.vue# 【新】覆盖率确认 + 存 CSV + 可打印报表 + 打印按钮
  App.vue         # 加"导出"路由；视图在"无考试打开"时给出提示门
package.json      # + @tauri-apps/plugin-dialog
```

**责任边界**：`grading-core` 仍不知道 Tauri 存在（`persist`/`export` 纯函数，`cargo test` 独立跑）；目录选择/文件写入的 IO 边界在 `commands.rs`；`export.rs` 只算数据与 CSV 字符串，不碰文件系统。

---

## Task 1: 持久化核心——每库单场 exam 句柄

**Files:**
- Create: `crates/grading-core/src/persist.rs`
- Modify: `crates/grading-core/src/models.rs`, `crates/grading-core/src/lib.rs`

**Interfaces:**
- Consumes: `Db`（`Db::open(path)` 已存在），`setup::create_exam`
- Produces:
  - `ensure_exam(db: &Db, name: &str, date: &str) -> anyhow::Result<i64>` —— 若库中已有 exam 行返回其 id（单场约束），否则新建并返回
  - `exam_info(db: &Db, exam_id: i64) -> anyhow::Result<ExamInfo>`
  - `struct ExamInfo { id: i64, name: String, date: String }`（serde）

- [ ] **Step 1: 定义 ExamInfo（追加 models.rs）**

```rust
// 追加到 crates/grading-core/src/models.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExamInfo { pub id: i64, pub name: String, pub date: String }
```

- [ ] **Step 2: 声明模块（追加 lib.rs）**

```rust
// crates/grading-core/src/lib.rs 追加两行（与现有 pub mod 并列）
pub mod persist;
pub mod export;
```

同时创建空的 `crates/grading-core/src/export.rs`（内容留空，Task 4 填），先让 `lib.rs` 能编译。

- [ ] **Step 3: 写失败测试**

```rust
// crates/grading-core/src/persist.rs
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
}
```

- [ ] **Step 4: 跑测试确认失败**

Run: `cargo test -p grading-core ensure_exam`
Expected: 编译失败——`ensure_exam`/`exam_info` 未定义。

- [ ] **Step 5: 实现 persist.rs**

```rust
// crates/grading-core/src/persist.rs （置于文件顶部）
use anyhow::Result;
use crate::{Db, ExamInfo, setup};

/// 每个 exam.db 只含一场考试：已有则返回其 id，否则新建。
pub fn ensure_exam(db: &Db, name: &str, date: &str) -> Result<i64> {
    let existing: Option<i64> = db.conn
        .query_row("SELECT id FROM exam ORDER BY id LIMIT 1", [], |r| r.get(0))
        .ok();
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
```

- [ ] **Step 6: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core persist`
Expected: PASS。

```bash
git add crates/grading-core/src/persist.rs crates/grading-core/src/export.rs crates/grading-core/src/models.rs crates/grading-core/src/lib.rs
git commit -m "feat(core): 每库单场 exam 句柄（ensure_exam/exam_info）"
```

---

## Task 2: 导出计算核心——覆盖率 / 三态 / 排名规则 / 缺考

**Files:**
- Modify: `crates/grading-core/src/export.rs`, `crates/grading-core/src/models.rs`

**Interfaces:**
- Consumes: `Db`, `setup::{list_problems,list_students}`, `ScoreState`
- Produces:
  - `build_export(db: &Db, exam_id: i64) -> anyhow::Result<ExportData>`
  - structs（全部 serde）：`Cell{ total: Option<i64>, state: String }`、`StudentRow{ student_id, name, exam_number: Option<String>, absent: bool, cells: Vec<Cell>, total: Option<i64>, complete: bool, rank: Option<i64> }`、`ProblemStat{ number, max_score, avg: Option<f64>, rate: Option<f64>, scored_count: i64 }`、`Coverage{ roster: i64, absent: i64, units_total: i64, graded: i64, flagged: i64, ungraded: i64 }`、`ExportData{ exam: ExamInfo, problem_numbers: Vec<i64>, problem_max: Vec<i64>, rows: Vec<StudentRow>, problem_stats: Vec<ProblemStat>, coverage: Coverage, ranking_available: bool }`
  - 语义：`Cell.state ∈ {"Graded","Flagged","Ungraded","Absent"}`；学生 `total` = 该生所有 `Graded|Flagged` 单元分之和（`Ungraded` 不计；缺考为 None）；`complete` = 非缺考且所有单元 `Graded`；`ranking_available` = 全部非缺考学生 `complete`；仅此时按 `total` 降序赋 `rank`（并列同名次），否则全 None。题目级 `avg/rate` 仅对该题非缺考、有分（Graded|Flagged）单元计算。

- [ ] **Step 1: 定义导出 struct（追加 models.rs）**

```rust
// 追加到 crates/grading-core/src/models.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cell { pub total: Option<i64>, pub state: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StudentRow {
    pub student_id: i64, pub name: String, pub exam_number: Option<String>,
    pub absent: bool, pub cells: Vec<Cell>, pub total: Option<i64>,
    pub complete: bool, pub rank: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProblemStat { pub number: i64, pub max_score: i64, pub avg: Option<f64>, pub rate: Option<f64>, pub scored_count: i64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coverage { pub roster: i64, pub absent: i64, pub units_total: i64, pub graded: i64, pub flagged: i64, pub ungraded: i64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportData {
    pub exam: ExamInfo, pub problem_numbers: Vec<i64>, pub problem_max: Vec<i64>,
    pub rows: Vec<StudentRow>, pub problem_stats: Vec<ProblemStat>,
    pub coverage: Coverage, pub ranking_available: bool,
}
```

- [ ] **Step 2: 写失败测试（覆盖三态/缺考/带洞不排名/得分率）**

```rust
// crates/grading-core/src/export.rs
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
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core export`
Expected: 编译失败——`build_export` 未定义。

- [ ] **Step 4: 实现 build_export**

```rust
// crates/grading-core/src/export.rs （置于文件顶部）
use anyhow::Result;
use crate::{Db, ExamInfo, Cell, StudentRow, ProblemStat, Coverage, ExportData,
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
```

- [ ] **Step 5: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core export`
Expected: 两个测试 PASS。

```bash
git add crates/grading-core/src/export.rs crates/grading-core/src/models.rs
git commit -m "feat(core): 导出计算——三态/临时总分/缺考/带洞不排名/题目得分率"
```

---

## Task 3: 导出 CSV 序列化

**Files:**
- Modify: `crates/grading-core/src/export.rs`

**Interfaces:**
- Consumes: `ExportData`（Task 2）
- Produces: `export_to_csv(data: &ExportData) -> String`
  - 列：`姓名,考号,题{n}...,总分[,排名]`（`排名` 列仅当 `ranking_available`）
  - 单元：`Graded`→数字；`Flagged`→`数字?`（带标记）；`Ungraded`→空；缺考行所有单元与总分→`缺考`
  - 值内含逗号/引号时按 RFC4180 加引号转义

- [ ] **Step 1: 写失败测试**

```rust
// 追加到 crates/grading-core/src/export.rs 的 tests 模块
#[test]
fn csv_marks_states_and_omits_rank_when_incomplete() {
    let db = Db::open_in_memory().unwrap();
    let exam = scenario(&db);
    let csv = export_to_csv(&build_export(&db, exam).unwrap());
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
fn csv_includes_rank_when_complete() {
    let db = Db::open_in_memory().unwrap();
    // 复用 ranking_appears_only_when_all_graded 的场景构造
    let exam = create_exam(&db, "E", "2026-07-02").unwrap();
    let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
    import_roster(&db, exam, &[RosterRow{name:"甲".into(),exam_number:None}]).unwrap();
    let s = list_students(&db, exam).unwrap();
    db.conn.execute("INSERT INTO page(exam_id,student_id,problem_number,image_path,seq,status) VALUES(?1,?2,1,'x.jpg',0,'labeled')", (exam, s[0].id)).unwrap();
    set_score(&db, s[0].id, p1, Some(7), None, ScoreState::Graded).unwrap();
    let csv = export_to_csv(&build_export(&db, exam).unwrap());
    assert_eq!(csv.lines().next().unwrap(), "姓名,考号,题1,总分,排名");
    assert!(csv.lines().any(|l| l == "甲,,7,7,1")); // 考号 None → 空
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p grading-core csv`
Expected: 编译失败——`export_to_csv` 未定义。

- [ ] **Step 3: 实现 export_to_csv**

```rust
// 追加到 crates/grading-core/src/export.rs （非 tests，文件主体）
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else { s.to_string() }
}

pub fn export_to_csv(data: &ExportData) -> String {
    let mut out = String::new();
    // 表头
    let mut header = vec!["姓名".to_string(), "考号".to_string()];
    for n in &data.problem_numbers { header.push(format!("题{n}")); }
    header.push("总分".to_string());
    if data.ranking_available { header.push("排名".to_string()); }
    out.push_str(&header.join(","));
    out.push('\n');

    for r in &data.rows {
        let mut f = vec![csv_field(&r.name), csv_field(r.exam_number.as_deref().unwrap_or(""))];
        for c in &r.cells {
            let cell = if r.absent { "缺考".to_string() }
                else { match c.state.as_str() {
                    "Graded" => c.total.map(|t| t.to_string()).unwrap_or_default(),
                    "Flagged" => c.total.map(|t| format!("{t}?")).unwrap_or_else(|| "?".into()),
                    _ => String::new(), // Ungraded 留空
                }};
            f.push(csv_field(&cell));
        }
        f.push(if r.absent { "缺考".to_string() } else { r.total.map(|t| t.to_string()).unwrap_or_default() });
        if data.ranking_available {
            f.push(r.rank.map(|x| x.to_string()).unwrap_or_default());
        }
        out.push_str(&f.join(","));
        out.push('\n');
    }
    out
}
```

- [ ] **Step 4: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core export`（含 csv）
Expected: 全 PASS。

```bash
git add crates/grading-core/src/export.rs
git commit -m "feat(core): 导出 CSV 序列化（状态标记/缺考/带洞省排名列）"
```

---

## Task 4: 持久化——AppState 改"当前打开的考试" + 考试生命周期命令

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`, `src-tauri/Cargo.toml`
- Create: `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: `grading_core::{Db, persist, fake, ...}`
- Produces（IPC 命令）:
  - `new_exam(dir: String) -> Result<i64, String>` —— 在 dir 下建 `images/` 与 `exam.db`，`ensure_exam`，设为当前打开，返回 exam_id
  - `open_exam(dir: String) -> Result<i64, String>` —— 打开 dir 下已有 `exam.db`
  - `seed_demo_exam(dir: String) -> Result<i64, String>` —— `new_exam` + 注入假数据（`fake::seed_fake_exam` 写入该库）
  - `current_exam() -> Result<Option<ExamInfo>, String>`
  - 既有命令改为作用于"当前打开的考试"：无考试打开时返回 `Err("no exam open")`
- 说明：`AppState` 从 `Mutex<Db>` 改为 `Mutex<Option<OpenExam>>`，`OpenExam{ db: Db, dir: PathBuf, exam_id: i64 }`。

> **决策（评审可否决）**：目录选择在**前端**用 `@tauri-apps/plugin-dialog` 的 `open({directory:true})` 完成，把选中的路径字符串传给这些命令——所以命令收 `dir: String`。dialog 是**插件命令**，需 `capabilities/default.json` 授权（不同于 app 命令默认放行）。

- [ ] **Step 1: 加 dialog 插件依赖**

```toml
# src-tauri/Cargo.toml [dependencies] 追加
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: 写 capabilities（授予 main 窗口 dialog 权限）**

```json
// src-tauri/capabilities/default.json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "阅卷辅助器默认能力：允许 main 窗口用文件对话框选考试目录",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:allow-open"]
}
```

- [ ] **Step 3: 重写 commands.rs 的状态与生命周期**

```rust
// src-tauri/src/commands.rs （替换 AppState 与相关命令；保留既有 setup/grading 命令签名，仅改为从 open exam 取 db）
use std::sync::Mutex;
use std::path::PathBuf;
use grading_core::{Db, ExamInfo, Problem, Preset, Student, GradingUnit, PageRef, ScoreState,
                   persist, setup, grading, fake};

pub struct OpenExam { pub db: Db, pub dir: PathBuf, pub exam_id: i64 }
pub struct AppState(pub Mutex<Option<OpenExam>>);

type R<T> = Result<T, String>;
fn e<E: std::fmt::Display>(x: E) -> String { x.to_string() }

/// 打开/新建一个考试目录：建 images/、开 exam.db、确定当前 exam_id，设为当前。
/// seed=true（演示）时由 fake::seed_fake_exam 建考试并返回其 id；否则 ensure_exam（单库单场）。
/// 注意：演示假定选的是空/新目录——不在已有考试的库上再 seed（否则会产生第二场，违反单库单场）。
fn open_dir(state: &tauri::State<AppState>, dir: String, seed: bool) -> R<i64> {
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(dir.join("images")).map_err(e)?;
    let db = Db::open(&dir.join("exam.db")).map_err(e)?;
    let has_exam: i64 = db.conn
        .query_row("SELECT count(*) FROM exam", [], |r| r.get(0)).map_err(e)?;
    let exam_id = if seed && has_exam == 0 {
        fake::seed_fake_exam(&db).map_err(e)?          // 建演示考试并返回其 id
    } else {
        persist::ensure_exam(&db, "未命名考试", "").map_err(e)?  // 已有则返回，否则新建空考试
    };
    *state.0.lock().map_err(e)? = Some(OpenExam { db, dir, exam_id });
    Ok(exam_id)
}

#[tauri::command] pub fn new_exam(state: tauri::State<AppState>, dir: String) -> R<i64> { open_dir(&state, dir, false) }
#[tauri::command] pub fn open_exam(state: tauri::State<AppState>, dir: String) -> R<i64> { open_dir(&state, dir, false) }
#[tauri::command] pub fn seed_demo_exam(state: tauri::State<AppState>, dir: String) -> R<i64> { open_dir(&state, dir, true) }

#[tauri::command]
pub fn current_exam(state: tauri::State<AppState>) -> R<Option<ExamInfo>> {
    let guard = state.0.lock().map_err(e)?;
    match guard.as_ref() {
        Some(oe) => Ok(Some(persist::exam_info(&oe.db, oe.exam_id).map_err(e)?)),
        None => Ok(None),
    }
}

// 小工具：取当前打开考试，无则报错
fn with_exam<T>(state: &tauri::State<AppState>, f: impl FnOnce(&OpenExam) -> anyhow::Result<T>) -> R<T> {
    let guard = state.0.lock().map_err(e)?;
    let oe = guard.as_ref().ok_or_else(|| "no exam open".to_string())?;
    f(oe).map_err(e)
}

#[tauri::command] pub fn list_problems(state: tauri::State<AppState>) -> R<Vec<Problem>> { with_exam(&state, |oe| setup::list_problems(&oe.db, oe.exam_id)) }
#[tauri::command] pub fn list_presets(state: tauri::State<AppState>, problem_id: i64) -> R<Vec<Preset>> { with_exam(&state, |oe| setup::list_presets(&oe.db, problem_id)) }
#[tauri::command] pub fn list_students(state: tauri::State<AppState>) -> R<Vec<Student>> { with_exam(&state, |oe| setup::list_students(&oe.db, oe.exam_id)) }
#[tauri::command] pub fn build_queue(state: tauri::State<AppState>, problem_number: i64) -> R<Vec<GradingUnit>> { with_exam(&state, |oe| grading::build_queue(&oe.db, oe.exam_id, problem_number)) }
#[tauri::command] pub fn student_pages(state: tauri::State<AppState>, student_id: i64) -> R<Vec<PageRef>> { with_exam(&state, |oe| grading::student_pages(&oe.db, student_id)) }

#[tauri::command]
pub fn set_score(state: tauri::State<AppState>, student_id: i64, problem_id: i64,
                 total: Option<i64>, preset_id: Option<i64>, state_str: String) -> R<()> {
    with_exam(&state, |oe| grading::set_score(&oe.db, student_id, problem_id, total, preset_id, ScoreState::from_str(&state_str)))
}
```

> 注意：`list_problems`/`list_students`/`build_queue` 去掉了 `exam_id` 入参（改用当前考试）；前端 api.ts 与调用点在 Task 6 同步更新。

- [ ] **Step 4: 更新 main.rs（注册插件与新命令，去掉内存库）**

```rust
// src-tauri/src/main.rs （替换）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
use std::sync::Mutex;
use commands::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            commands::new_exam, commands::open_exam, commands::seed_demo_exam, commands::current_exam,
            commands::list_problems, commands::list_presets, commands::list_students,
            commands::build_queue, commands::set_score, commands::student_pages
            // 注：导出命令在 Task 5 追加进本列表
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: 编译验证**

Run: `cargo build`
Expected: 成功。首次拉取 `tauri-plugin-dialog` 会编译一批依赖，属正常。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/
git commit -m "feat(persist): AppState 改当前打开考试 + 新建/打开/演示考试命令 + dialog 能力"
```

---

## Task 5: 导出 IPC —— 汇总命令 + 保存 CSV 真文件

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `grading_core::export::{build_export, export_to_csv}`
- Produces:
  - `export_summary() -> Result<ExportData, String>`（作用于当前考试）
  - `save_csv(path: String) -> Result<(), String>` —— 计算当前考试 CSV 并写到 path（路径来自前端保存对话框）

- [ ] **Step 1: 追加导出命令（commands.rs）**

```rust
// 追加到 src-tauri/src/commands.rs
use grading_core::{ExportData, export};

#[tauri::command]
pub fn export_summary(state: tauri::State<AppState>) -> R<ExportData> {
    with_exam(&state, |oe| export::build_export(&oe.db, oe.exam_id))
}

#[tauri::command]
pub fn save_csv(state: tauri::State<AppState>, path: String) -> R<()> {
    let csv = with_exam(&state, |oe| Ok(export::export_to_csv(&export::build_export(&oe.db, oe.exam_id)?)))?;
    // 加 UTF-8 BOM，Excel 直接识别中文
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(csv.as_bytes());
    std::fs::write(&path, bytes).map_err(e)
}
```

- [ ] **Step 2: 注册两命令（main.rs invoke_handler 追加）**

```rust
// src-tauri/src/main.rs 的 generate_handler! 列表追加：
            , commands::export_summary, commands::save_csv
```

- [ ] **Step 3: 编译验证 + 提交**

Run: `cargo build`
Expected: 成功。

```bash
git add src-tauri/
git commit -m "feat(export): export_summary 与 save_csv（BOM+UTF8）IPC 命令"
```

---

## Task 6: 前端类型与 api —— 同步命令签名变化 + 导出/考试封装

**Files:**
- Modify: `src/types.ts`, `src/api.ts`, `src/views/SetupView.vue`, `src/views/GradeView.vue`
- Modify: `package.json`

**Interfaces:**
- Produces: `newExam/openExam/seedDemoExam/currentExam/exportSummary/saveCsv` 的类型化封装；`ExamInfo`/`ExportData` 等 TS 类型；`listProblems/listStudents/buildQueue` 去掉 examId 入参。

- [ ] **Step 1: 装 dialog 前端插件**

Run: `npm install @tauri-apps/plugin-dialog@^2`
在 `package.json` dependencies 出现 `@tauri-apps/plugin-dialog`。

- [ ] **Step 2: 追加/修改 types.ts**

```ts
// 追加到 src/types.ts
export interface ExamInfo { id: number; name: string; date: string }
export interface Cell { total: number | null; state: "Graded" | "Flagged" | "Ungraded" | "Absent" }
export interface StudentRow {
  student_id: number; name: string; exam_number: string | null;
  absent: boolean; cells: Cell[]; total: number | null; complete: boolean; rank: number | null;
}
export interface ProblemStat { number: number; max_score: number; avg: number | null; rate: number | null; scored_count: number }
export interface Coverage { roster: number; absent: number; units_total: number; graded: number; flagged: number; ungraded: number }
export interface ExportData {
  exam: ExamInfo; problem_numbers: number[]; problem_max: number[];
  rows: StudentRow[]; problem_stats: ProblemStat[]; coverage: Coverage; ranking_available: boolean;
}
```

- [ ] **Step 3: 改 api.ts（命令签名变化 + 新封装）**

```ts
// src/api.ts （替换受影响部分）
import { invoke } from "@tauri-apps/api/core";
import type { Problem, Preset, Student, GradingUnit, PageRef, ScoreState, ExamInfo, ExportData } from "./types";

// 考试生命周期
export const newExam = (dir: string) => invoke<number>("new_exam", { dir });
export const openExam = (dir: string) => invoke<number>("open_exam", { dir });
export const seedDemoExam = (dir: string) => invoke<number>("seed_demo_exam", { dir });
export const currentExam = () => invoke<ExamInfo | null>("current_exam");

// 作用于当前考试（不再传 examId）
export const listProblems = () => invoke<Problem[]>("list_problems");
export const listPresets = (problemId: number) => invoke<Preset[]>("list_presets", { problemId });
export const listStudents = () => invoke<Student[]>("list_students");
export const buildQueue = (problemNumber: number) => invoke<GradingUnit[]>("build_queue", { problemNumber });
export const setScore = (studentId: number, problemId: number, total: number | null,
                         presetId: number | null, stateStr: ScoreState) =>
  invoke<void>("set_score", { studentId, problemId, total, presetId, stateStr });
export const studentPages = (studentId: number) => invoke<PageRef[]>("student_pages", { studentId });

// 导出
export const exportSummary = () => invoke<ExportData>("export_summary");
export const saveCsv = (path: string) => invoke<void>("save_csv", { path });
```

- [ ] **Step 4: 更新 SetupView.vue —— 用 dialog 开/建/演示考试**

```vue
<!-- src/views/SetupView.vue （替换 <script setup> 的载入逻辑与模板顶部按钮） -->
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { newExam, openExam, seedDemoExam, currentExam, listProblems, listPresets, listStudents } from "../api";
import type { Problem, Preset, Student, ExamInfo } from "../types";
import { NButton, NCard, NDataTable, NAlert, NSpace, NTag } from "naive-ui";

const exam = ref<ExamInfo | null>(null);
const problems = ref<Problem[]>([]);
const presetsByProblem = ref<Record<number, Preset[]>>({});
const students = ref<Student[]>([]);
const errorMsg = ref("");

async function refresh() {
  exam.value = await currentExam();
  if (!exam.value) { problems.value = []; students.value = []; presetsByProblem.value = {}; return; }
  problems.value = await listProblems();
  const np: Record<number, Preset[]> = {};
  for (const p of problems.value) np[p.id] = await listPresets(p.id);
  presetsByProblem.value = np;
  students.value = await listStudents();
}

async function pickDir(): Promise<string | null> {
  const d = await open({ directory: true, multiple: false, title: "选择考试目录" });
  return typeof d === "string" ? d : null;
}
async function doNew()  { await withDir(newExam); }
async function doOpen() { await withDir(openExam); }
async function doDemo() { await withDir(seedDemoExam); }
async function withDir(fn: (dir: string) => Promise<number>) {
  errorMsg.value = "";
  try { const dir = await pickDir(); if (!dir) return; await fn(dir); await refresh(); }
  catch (e) { errorMsg.value = String(e); }
}
onMounted(refresh);
</script>

<template>
  <section style="padding:16px; font-family:ui-monospace,monospace;">
    <n-space>
      <n-button @click="doNew">新建考试…</n-button>
      <n-button @click="doOpen">打开考试…</n-button>
      <n-button type="primary" @click="doDemo">新建演示考试…</n-button>
    </n-space>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px" />
    <p v-if="exam">当前：{{ exam.name }}（exam_id={{ exam.id }}）</p>
    <p v-else>未打开考试。点上面按钮选一个目录。</p>

    <div v-for="p in problems" :key="p.id" style="margin:8px 0;">
      <h3>题{{ p.number }} · {{ p.title }}
        <n-tag size="small">满分 {{ p.max_score }}</n-tag>
      </h3>
      <ul><li v-for="pr in presetsByProblem[p.id]" :key="pr.id">键 {{ pr.slot }} → {{ pr.label }} = {{ pr.points }}</li></ul>
    </div>
    <n-card v-if="students.length" :title="`花名册（${students.length} 人）`">
      <n-data-table :columns="[{title:'姓名',key:'name'},{title:'考号',key:'exam_number'}]" :data="students" />
    </n-card>
  </section>
</template>
```

- [ ] **Step 5: 更新 GradeView.vue 调用点（去掉 examId 入参）**

在 `src/views/GradeView.vue` 中，把 `buildQueue(examId, problemNumber.value)` 改为 `buildQueue(problemNumber.value)`，删除 `const examId = 1;`。其余逻辑不变（当前仍固定批第 1 题）。若队列为空（无考试打开/未 seed），既有的空态提示照常显示。

```ts
// GradeView.vue：loadQueue 内
queue.value = await buildQueue(problemNumber.value);
```

- [ ] **Step 6: 编译验证 + 提交**

Run: `npm run build`（vue-tsc + vite）
Expected: 通过。

```bash
git add src/ package.json package-lock.json
git commit -m "feat(front): 考试开/建/演示（dialog）+ api 同步当前考试语义 + 导出封装"
```

---

## Task 7: 前端导出视图 —— 覆盖率确认 + 存 CSV + 可打印报表（PDF）

**Files:**
- Create: `src/views/ExportView.vue`
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: `exportSummary`, `saveCsv`（api.ts），`@tauri-apps/plugin-dialog` 的 `save`
- Produces: 一个导出界面——先算 `ExportData` 展示**覆盖率确认**（缺考/已判/存疑/未判、是否可排名），确认后可（a）存 CSV 真文件、（b）渲染可打印 HTML 报表并 `window.print()` 另存 PDF。

> **决策（评审可否决）**：PDF 不引 Rust 库，走"报表区域用 print CSS 排版 → `window.print()` → 系统打印对话框另存为 PDF"。中文字体由 webview 提供，无嵌入坑。若你要"一键直接生成 PDF 文件"，需另评估（如无头打印/Rust 库+字体），本片不做。

- [ ] **Step 1: 实现 ExportView.vue**

```vue
<!-- src/views/ExportView.vue -->
<script setup lang="ts">
import { ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { exportSummary, saveCsv } from "../api";
import type { ExportData } from "../types";
import { NButton, NAlert, NSpace, NDataTable } from "naive-ui";

const data = ref<ExportData | null>(null);
const errorMsg = ref("");
const okMsg = ref("");

async function load() {
  errorMsg.value = ""; okMsg.value = "";
  try { data.value = await exportSummary(); }
  catch (e) { errorMsg.value = String(e); }
}
async function doSaveCsv() {
  errorMsg.value = ""; okMsg.value = "";
  try {
    const path = await save({ title: "保存成绩表 CSV", defaultPath: "成绩表.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await saveCsv(path);
    okMsg.value = `已保存：${path}`;
  } catch (e) { errorMsg.value = String(e); }
}
function printReport() { window.print(); }

// 展示用：单元文本（与 CSV 规则一致）
function cellText(r: ExportData["rows"][number], i: number): string {
  if (r.absent) return "缺考";
  const c = r.cells[i];
  if (c.state === "Graded") return c.total?.toString() ?? "";
  if (c.state === "Flagged") return c.total != null ? `${c.total}?` : "?";
  return ""; // Ungraded 留空
}
</script>

<template>
  <section class="export">
    <n-space>
      <n-button @click="load">计算成绩汇总</n-button>
      <n-button v-if="data" @click="doSaveCsv">保存 CSV…</n-button>
      <n-button v-if="data" @click="printReport">打印 / 导出 PDF</n-button>
    </n-space>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px"/>
    <n-alert v-if="okMsg" type="success" :title="okMsg" closable @close="okMsg=''" style="margin-top:8px"/>

    <!-- 覆盖率确认 -->
    <div v-if="data" class="coverage">
      花名册 {{ data.coverage.roster }} 人，缺考 {{ data.coverage.absent }}；
      单元 已判 {{ data.coverage.graded }} / 存疑 {{ data.coverage.flagged }} / 未判 {{ data.coverage.ungraded }}
      （共 {{ data.coverage.units_total }}）。
      <b v-if="!data.ranking_available">尚有未判/存疑 → 不出排名（先批完再排）。</b>
      <b v-else>全部已判 → 含排名。</b>
    </div>

    <!-- 可打印报表 -->
    <div v-if="data" id="report" class="report">
      <h2>{{ data.exam.name }} 成绩表</h2>
      <table>
        <thead><tr>
          <th>姓名</th><th>考号</th>
          <th v-for="n in data.problem_numbers" :key="n">题{{ n }}</th>
          <th>总分</th><th v-if="data.ranking_available">排名</th>
        </tr></thead>
        <tbody>
          <tr v-for="r in data.rows" :key="r.student_id" :class="{absent:r.absent}">
            <td>{{ r.name }}</td><td>{{ r.exam_number ?? "" }}</td>
            <td v-for="(_, i) in data.problem_numbers" :key="i">{{ cellText(r, i) }}</td>
            <td>{{ r.absent ? "缺考" : (r.total ?? "") }}</td>
            <td v-if="data.ranking_available">{{ r.rank ?? "" }}</td>
          </tr>
        </tbody>
      </table>
      <h3>各题得分率</h3>
      <table>
        <thead><tr><th>题</th><th>满分</th><th>平均</th><th>得分率</th><th>已评</th></tr></thead>
        <tbody>
          <tr v-for="ps in data.problem_stats" :key="ps.number">
            <td>题{{ ps.number }}</td><td>{{ ps.max_score }}</td>
            <td>{{ ps.avg != null ? ps.avg.toFixed(1) : "—" }}</td>
            <td>{{ ps.rate != null ? (ps.rate*100).toFixed(0)+"%" : "—" }}</td>
            <td>{{ ps.scored_count }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.export { padding: 16px; font-family: ui-monospace, monospace; }
.coverage { margin: 12px 0; padding: 8px; border: 1px solid #444; }
.report table { border-collapse: collapse; margin: 8px 0; width: 100%; }
.report th, .report td { border: 1px solid #555; padding: 2px 8px; text-align: right; }
.report th:first-child, .report td:first-child { text-align: left; }
.report tr.absent { color: #999; }
/* 打印：只留报表，去掉按钮/背景 */
@media print {
  .export > .n-space, .export > .n-alert, .coverage { display: none !important; }
  .report th, .report td { border-color: #000; color: #000; }
}
</style>
```

- [ ] **Step 2: App.vue 加"导出"路由**

```vue
<!-- src/App.vue：script 的 route 联合类型加 "export"，template nav 加按钮与视图 -->
```
```ts
// route 类型
const route = ref<"setup" | "grade" | "export">("setup");
```
```html
<!-- nav 内追加 -->
<button @click="route = 'export'">导出</button>
<!-- 视图区追加 -->
<ExportView v-else-if="route === 'export'" />
```
并 `import ExportView from "./views/ExportView.vue";`（GradeView 的 `v-else` 改为 `v-else-if="route==='grade'"`，保证三视图互斥）。

- [ ] **Step 3: 构建验证**

Run: `npm run build`
Expected: 通过。

- [ ] **Step 4: 真机验收（CDP 驱动，同 M1 做法）**

`npm run tauri dev`：
1. 考试设置 → "新建演示考试…" 选一个空目录 → 确认题/档位/花名册渲染、该目录下生成了 `exam.db` 与 `images/`。
2. 判分 → 批几份（含按 F 存疑一份、留一份未判）。
3. 导出 → "计算成绩汇总" → 覆盖率显示"未判/存疑 → 不出排名"；"保存 CSV…" 选路径 → 用记事本/Excel 打开，确认中文正常、存疑标 `?`、未判留空、无排名列。
4. 回判分把剩余全判完 → 导出重算 → 覆盖率变"全部已判 → 含排名"，报表出现排名列；"打印 / 导出 PDF" 弹系统打印框、报表版式只剩表格。
报告实际观察到的结果；若无法驱动 GUI，则至少 `npm run build` 通过并说明哪些交互未运行时验证。

- [ ] **Step 5: 提交**

```bash
git add src/views/ExportView.vue src/App.vue
git commit -m "feat(export): 导出视图——覆盖率确认/存CSV/可打印报表(PDF)"
```

---

## Self-Review（对照 spec §4/§9 的覆盖检查）

- **每场一个自包含 exam.db 目录**：Task 4 `open_dir`（建 images/ + exam.db）✓；spec §4.2 存储组织 ✓
- **每库单场**：Task 1 `ensure_exam` ✓
- **三态 + 缺考，绝不静默填 0**：Task 2 `build_export`（Ungraded→None、Absent 分离）✓；Task 3 CSV（Ungraded 留空、缺考整行标）✓
- **存疑临时分计入临时总分、未判不计**：Task 2（`matches!(Graded|Flagged)` 才累加）+ 测试 ✓
- **带洞不出排名**：Task 2 `ranking_available` + 测试、Task 3 省排名列 + 测试 ✓
- **题目级得分率**：Task 2 `problem_stats`（非缺考有分单元均值/满分）+ 测试 ✓
- **导出前覆盖率确认**：Task 7 覆盖率区块 ✓
- **CSV/PDF**：Task 3 + Task 5 CSV 真文件（BOM）✓；Task 7 可打印 HTML 报表（PDF via print）✓
- **数据不出本地**：全程本地文件/库，无网络 ✓
- **持久化后的既有视图**：Task 6 SetupView/GradeView 同步当前考试语义 ✓

**已知的、有意的后续缺口（非占位）**：完整 Exam Setup 手工录入表单（本片仍以演示 seed + 只读展示 + 打开已有库为主，手工增删题/编辑档位推迟）；M2 Label/M4 Ingest（真实图导入与标注）；GradeView 仍固定批第 1 题（选题随 M2/M4 一并解决）；一键无头生成 PDF（本片走 print）。这些进入后续兄弟计划。
