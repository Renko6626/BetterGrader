# 阅卷辅助器 · Ingest(M4) + Label(M2) + 真图/选题 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让工具吃真实扫描图——指着一个图片文件夹导入、靠姓名页定边界从花名册标注、全键盘批任意题、导出，全链路闭环。

**Architecture:** `grading-core` 纯逻辑加两块可测单元：自然排序 + ingest 建行、Label 落库与确认总表(§7.0 页数校验/缺考)。`src-tauri` 加 `ingest_folder`(fs 枚举+拷贝)与 `read_image`(读字节)命令。前端：`read_image`→blob 显示真图、纯 `useLabelKeys` reducer(单测)、LabelView(浏览器+花名册选人+确认总表)、GradeView 补选题+真图。

**Tech Stack:** 沿用(Tauri v2 · rusqlite bundled · Vue3+TS+Vite · Vitest · naive-ui · @tauri-apps/plugin-dialog)。

## Global Constraints

- 一场考试 = 自包含目录 `/某场/{images/, exam.db}`；`page.image_path` 只存**文件名**(相对 images/)，显示时解析为 `<dir>/images/<文件名>`——目录可拎走。数据不出本地。
- Ingest：`jpg/jpeg/png/webp`(大小写不敏感)，**文件名自然排序**定 seq，**拷贝**进 images/(原件不动)，建**未标注 page**(`student_id/problem_number=NULL, status='ingested'`)；**再导追加**(seq 续)。
- 真图：`read_image(filename)` 读字节 → 前端 blob URL；路径**限定在 images/ 内**，拒绝越界。缺图降级占位。
- Label：姓名页 `S`定边界→花名册选人(搜姓名/键考号；查无此人→临时加/待匹配)，姓名页 `problem_number=0`；答题页自动编号 1..N，`C`接上题(不进)、`N`跳题(counter 进、不消耗页)；**§7.0 页数≠N 强制指认**(确认总表红标)；**增量写库**(每步落 page 行，可续标)；**缺考**=花名册有、无 page 绑定。
- 判分：GradeView 加选题(替固定第1题)，左图换 `read_image` 真图。
- **零 schema 改动**(page 的 student/题号本就可空，status 承载 ingested/labeled)。
- 三态/导出/单库单场等既有约束不变；AI 永不写 score.total。
- **凡涉原生对话框/capability 的路径 CDP 难驱动**——验收里显式标"需人工点一次 + 单测/ACL 兜底"，不谎报。

---

## File Structure

```
crates/grading-core/src/
  ingest.rs   # 【新】sort_scan_order(纯,可测) + next_seq + add_ingested_page
  label.rs    # 【新】PageRow/list_pages/set_page_label/add_student + LabelSummary/labeling_summary(§7.0/缺考)
  models.rs   # 追加 PageRow / StackRow / LabelSummary
  lib.rs      # 追加 pub mod ingest; pub mod label;
src-tauri/src/
  commands.rs # 追加 ingest_folder / read_image / label 命令(list_pages/set_page_label/add_student/labeling_summary)
  main.rs     # 注册新命令
src/
  composables/useLabelKeys.ts       # 【新】纯 Label reducer
  composables/useLabelKeys.test.ts  # 【新】Vitest
  composables/useImage.ts           # 【新】read_image→blob URL(带 revoke)
  views/LabelView.vue               # 【新】单图浏览器+花名册选人+确认总表
  views/SetupView.vue               # 改：加"导入图片文件夹"入口
  views/GradeView.vue               # 改：题目选择器 + 真图
  App.vue                           # 加"标注"路由
  api.ts / types.ts                 # 同步
```

**责任边界**：`grading-core` 仍无 fs/Tauri（`sort_scan_order` 纯函数；ingest 的 fs 拷贝在 command 层，core 只管建行/查询）；`useLabelKeys` 无副作用(输入 state+key→state+effect)，LabelView 执行 effect(落库/翻页)。

---

## Task 1: grading-core ingest —— 自然排序 + 建未标注 page

**Files:**
- Create: `crates/grading-core/src/ingest.rs`
- Modify: `crates/grading-core/src/lib.rs`

**Interfaces:**
- Consumes: `Db`
- Produces:
  - `sort_scan_order(files: Vec<String>) -> Vec<String>`（纯：数字段按数值、其余按字典，自然序）
  - `next_seq(db, exam_id: i64) -> anyhow::Result<i64>`（当前 max(seq)+1，空则 0）
  - `add_ingested_page(db, exam_id: i64, filename: &str, seq: i64) -> anyhow::Result<i64>`（插未标注 page，返回 page id）

- [ ] **Step 1: 声明模块**

```rust
// crates/grading-core/src/lib.rs 追加（与现有 pub mod 并列）
pub mod ingest;
pub mod label;
```
同时建空 `crates/grading-core/src/label.rs`（Task 3 填），先让 lib 编译。

- [ ] **Step 2: 写失败测试**

```rust
// crates/grading-core/src/ingest.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, setup::create_exam};

    #[test]
    fn natural_sort_orders_numbers_by_value() {
        let got = sort_scan_order(vec![
            "10.jpg".into(), "2.jpg".into(), "1.jpg".into(), "img11.png".into(), "img2.png".into(),
        ]);
        assert_eq!(got, vec!["1.jpg", "2.jpg", "10.jpg", "img2.png", "img11.png"]);
    }

    #[test]
    fn next_seq_and_add_ingested_page() {
        let db = Db::open_in_memory().unwrap();
        let exam = create_exam(&db, "E", "2026-07-03").unwrap();
        assert_eq!(next_seq(&db, exam).unwrap(), 0);
        let id = add_ingested_page(&db, exam, "a.jpg", 0).unwrap();
        assert!(id > 0);
        add_ingested_page(&db, exam, "b.jpg", 1).unwrap();
        assert_eq!(next_seq(&db, exam).unwrap(), 2); // max(seq)=1 → next 2
        // 未标注：student_id/problem_number 为 NULL，status='ingested'
        let (sid, pn, st): (Option<i64>, Option<i64>, String) = db.conn.query_row(
            "SELECT student_id, problem_number, status FROM page WHERE image_path='a.jpg'",
            [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((sid, pn, st.as_str()), (None, None, "ingested"));
    }
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core ingest`
Expected: 编译失败——函数未定义。

- [ ] **Step 4: 实现 ingest.rs**

```rust
// crates/grading-core/src/ingest.rs （文件顶部）
use anyhow::Result;
use crate::Db;

/// 自然排序：把字符串切成 数字块/非数字块 交替的 key，数字块按数值比较。
fn nat_key(s: &str) -> Vec<(bool, u64, String)> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() { num.push(d); chars.next(); } else { break; }
            }
            let v = num.parse::<u64>().unwrap_or(u64::MAX);
            out.push((true, v, String::new()));
        } else {
            let mut seg = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() { break; } else { seg.push(d.to_ascii_lowercase()); chars.next(); }
            }
            out.push((false, 0, seg));
        }
    }
    out
}

pub fn sort_scan_order(mut files: Vec<String>) -> Vec<String> {
    files.sort_by(|a, b| nat_key(a).cmp(&nat_key(b)));
    files
}

pub fn next_seq(db: &Db, exam_id: i64) -> Result<i64> {
    let n: Option<i64> = db.conn.query_row(
        "SELECT MAX(seq) FROM page WHERE exam_id=?1", [exam_id], |r| r.get(0))?;
    Ok(n.map(|x| x + 1).unwrap_or(0))
}

pub fn add_ingested_page(db: &Db, exam_id: i64, filename: &str, seq: i64) -> Result<i64> {
    db.conn.execute(
        "INSERT INTO page(exam_id, student_id, problem_number, image_path, seq, status)
         VALUES(?1, NULL, NULL, ?2, ?3, 'ingested')",
        (exam_id, filename, seq))?;
    Ok(db.conn.last_insert_rowid())
}
```

- [ ] **Step 5: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core ingest`
Expected: 两测试 PASS。

```bash
git add crates/grading-core/src/ingest.rs crates/grading-core/src/label.rs crates/grading-core/src/lib.rs
git commit -m "feat(core): ingest——自然排序 + 建未标注 page"
```

---

## Task 2: grading-core label —— 页查询/标注写入/学生追加

**Files:**
- Modify: `crates/grading-core/src/label.rs`, `crates/grading-core/src/models.rs`

**Interfaces:**
- Consumes: `Db`
- Produces:
  - `struct PageRow { id, seq, image_path, student_id: Option<i64>, problem_number: Option<i64>, status: String }`（serde）
  - `list_pages(db, exam_id) -> Result<Vec<PageRow>>`（按 seq）
  - `set_page_label(db, page_id, student_id: Option<i64>, problem_number: Option<i64>) -> Result<()>`（写标注；student_id 为 Some 时 status='labeled'，为 None 时回退 'ingested'）
  - `add_student(db, exam_id, name: &str, exam_number: Option<&str>) -> Result<i64>`（追加学生，roster_order = 当前 max+1）

- [ ] **Step 1: 定义 PageRow（追加 models.rs）**

```rust
// 追加到 crates/grading-core/src/models.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageRow {
    pub id: i64, pub seq: i64, pub image_path: String,
    pub student_id: Option<i64>, pub problem_number: Option<i64>, pub status: String,
}
```

- [ ] **Step 2: 写失败测试**

```rust
// crates/grading-core/src/label.rs
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
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core label::tests::list_set_label`
Expected: 编译失败——未定义。

- [ ] **Step 4: 实现 label.rs（本任务部分）**

```rust
// crates/grading-core/src/label.rs （文件顶部）
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
```

- [ ] **Step 5: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core label`
Expected: PASS。

```bash
git add crates/grading-core/src/label.rs crates/grading-core/src/models.rs
git commit -m "feat(core): label——页查询/标注写入/学生追加"
```

---

## Task 3: grading-core 确认总表 —— §7.0 页数校验 + 缺考

**Files:**
- Modify: `crates/grading-core/src/label.rs`, `crates/grading-core/src/models.rs`

**Interfaces:**
- Consumes: `Db`、`list_pages`、`setup::{list_problems,list_students}`
- Produces:
  - `struct StackRow { student_id, student_name, answer_pages: i64, problem_count: i64, count_ok: bool }`
  - `struct LabelSummary { stacks: Vec<StackRow>, absent_students: Vec<Student>, unlabeled_pages: i64 }`
  - `labeling_summary(db, exam_id) -> Result<LabelSummary>`
  - 语义：`answer_pages` = 该生 `problem_number >= 1` 的 page 数；`problem_count` = 本场题数 N；`count_ok = (answer_pages == N)`（§7.0）；`absent_students` = 无任何 page 绑定的花名册学生；`unlabeled_pages` = `student_id IS NULL` 的 page 数。

- [ ] **Step 1: 定义 struct（追加 models.rs）**

```rust
// 追加到 crates/grading-core/src/models.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StackRow { pub student_id: i64, pub student_name: String, pub answer_pages: i64, pub problem_count: i64, pub count_ok: bool }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabelSummary { pub stacks: Vec<StackRow>, pub absent_students: Vec<Student>, pub unlabeled_pages: i64 }
```

- [ ] **Step 2: 写失败测试**

```rust
// 追加到 crates/grading-core/src/label.rs 的 tests 模块
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
    ]).unwrap();
    let s: Vec<i64> = crate::setup::list_students(&db, exam).unwrap().iter().map(|x| x.id).collect();
    // 甲：姓名页(0) + 题1 + 题2 = 答题页 2 == N ✓
    for (seq,(pn)) in [(0,0),(1,1),(2,2)] { let id=add_ingested_page(&db,exam,&format!("j{seq}.jpg"),seq).unwrap(); set_page_label(&db,id,Some(s[0]),Some(pn)).unwrap(); }
    // 乙：姓名页 + 只有题1 = 答题页 1 != 2 ✗（§7.0 报警）
    for (seq,(pn)) in [(3,0),(4,1)] { let id=add_ingested_page(&db,exam,&format!("y{seq}.jpg"),seq).unwrap(); set_page_label(&db,id,Some(s[1]),Some(pn)).unwrap(); }
    // 一张未标注
    add_ingested_page(&db, exam, "x.jpg", 9).unwrap();

    let sum = labeling_summary(&db, exam).unwrap();
    let jia = sum.stacks.iter().find(|r| r.student_id==s[0]).unwrap();
    assert_eq!((jia.answer_pages, jia.problem_count, jia.count_ok), (2, 2, true));
    let yi = sum.stacks.iter().find(|r| r.student_id==s[1]).unwrap();
    assert_eq!((yi.answer_pages, yi.count_ok), (1, false));   // 页数≠N
    assert!(sum.stacks.iter().all(|r| r.student_id != s[2])); // 丙无 page → 不在 stacks
    assert_eq!(sum.absent_students.len(), 1);                 // 丙 缺考
    assert_eq!(sum.absent_students[0].name, "丙");
    assert_eq!(sum.unlabeled_pages, 1);
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core summary_counts`
Expected: 编译失败——`labeling_summary` 未定义。

- [ ] **Step 4: 实现 labeling_summary**

```rust
// 追加到 crates/grading-core/src/label.rs（非 tests）
use crate::{StackRow, LabelSummary, Student, setup};

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
```

- [ ] **Step 5: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core label`
Expected: 全 PASS。

```bash
git add crates/grading-core/src/label.rs crates/grading-core/src/models.rs
git commit -m "feat(core): 确认总表——§7.0 页数校验 + 缺考 + 未标注计数"
```

---

## Task 4: src-tauri —— ingest_folder / read_image / label 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `grading_core::{ingest, label, PageRow, LabelSummary, Student}`；`OpenExam{db,dir,exam_id}`、`with_exam`。
- Produces（命令）:
  - `ingest_folder(src_dir: String) -> Result<usize, String>`（枚举图片、自然排序、拷进 images/、建未标注 page；返回导入张数）
  - `read_image(filename: String) -> Result<Vec<u8>, String>`（在当前考试 images/ 读该文件字节；路径限定 images/ 内）
  - `list_pages() -> Result<Vec<PageRow>>`、`set_page_label(page_id, student_id, problem_number)`、`add_student(name, exam_number)`、`labeling_summary() -> LabelSummary`

- [ ] **Step 1: 实现命令（commands.rs 追加）**

```rust
// 追加到 src-tauri/src/commands.rs
use grading_core::{ingest, label, PageRow, LabelSummary, Student};
use std::path::PathBuf;

#[tauri::command]
pub fn ingest_folder(state: tauri::State<AppState>, src_dir: String) -> R<usize> {
    let guard = state.0.lock().map_err(e)?;
    let oe = guard.as_ref().ok_or_else(|| "no exam open".to_string())?;
    let src = PathBuf::from(&src_dir);
    let images = oe.dir.join("images");
    std::fs::create_dir_all(&images).map_err(e)?;

    // 枚举图片文件名（仅文件、扩展名匹配）
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&src).map_err(e)? {
        let entry = entry.map_err(e)?;
        if !entry.file_type().map_err(e)?.is_file() { continue; }
        let name = entry.file_name().to_string_lossy().to_string();
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") { names.push(name); }
    }
    let ordered = ingest::sort_scan_order(names);

    let mut seq = ingest::next_seq(&oe.db, oe.exam_id).map_err(e)?;
    let mut count = 0usize;
    for name in ordered {
        // 目标文件名避免冲突：已存在则加 seq 前缀
        let mut dest_name = name.clone();
        if images.join(&dest_name).exists() { dest_name = format!("{seq}_{name}"); }
        std::fs::copy(src.join(&name), images.join(&dest_name)).map_err(e)?;
        ingest::add_ingested_page(&oe.db, oe.exam_id, &dest_name, seq).map_err(e)?;
        seq += 1; count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub fn read_image(state: tauri::State<AppState>, filename: String) -> R<Vec<u8>> {
    let guard = state.0.lock().map_err(e)?;
    let oe = guard.as_ref().ok_or_else(|| "no exam open".to_string())?;
    // 路径限定 images/ 内：拒绝分隔符/上跳
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("非法文件名".into());
    }
    let path = oe.dir.join("images").join(&filename);
    std::fs::read(&path).map_err(e)
}

#[tauri::command]
pub fn list_pages(state: tauri::State<AppState>) -> R<Vec<PageRow>> {
    with_exam(&state, |oe| label::list_pages(&oe.db, oe.exam_id))
}
#[tauri::command]
pub fn set_page_label(state: tauri::State<AppState>, page_id: i64, student_id: Option<i64>, problem_number: Option<i64>) -> R<()> {
    with_exam(&state, |oe| label::set_page_label(&oe.db, page_id, student_id, problem_number))
}
#[tauri::command]
pub fn add_student(state: tauri::State<AppState>, name: String, exam_number: Option<String>) -> R<i64> {
    with_exam(&state, |oe| label::add_student(&oe.db, oe.exam_id, &name, exam_number.as_deref()))
}
#[tauri::command]
pub fn labeling_summary(state: tauri::State<AppState>) -> R<LabelSummary> {
    with_exam(&state, |oe| label::labeling_summary(&oe.db, oe.exam_id))
}
```

- [ ] **Step 2: 注册命令（main.rs generate_handler! 追加）**

```rust
            , commands::ingest_folder, commands::read_image, commands::list_pages,
              commands::set_page_label, commands::add_student, commands::labeling_summary
```

- [ ] **Step 3: 编译验证 + 提交**

Run: `cargo build`
Expected: 成功。

```bash
git add src-tauri/
git commit -m "feat(ipc): ingest_folder/read_image/label 命令"
```

---

## Task 5: 前端 api/types 同步 + 真图 composable

**Files:**
- Modify: `src/types.ts`, `src/api.ts`
- Create: `src/composables/useImage.ts`
- Modify: `package.json`（若 @tauri-apps/plugin-dialog 未装则装；M3 已装可跳过）

**Interfaces:**
- Produces: TS 类型 `PageRow/StackRow/LabelSummary`；封装 `ingestFolder/readImage/listPages/setPageLabel/addStudent/labelingSummary`；`useImage()` 把 filename → blob URL（含 revoke）。

- [ ] **Step 1: 追加 types.ts**

```ts
// 追加到 src/types.ts
export interface PageRow { id: number; seq: number; image_path: string; student_id: number | null; problem_number: number | null; status: string }
export interface StackRow { student_id: number; student_name: string; answer_pages: number; problem_count: number; count_ok: boolean }
export interface LabelSummary { stacks: StackRow[]; absent_students: Student[]; unlabeled_pages: number }
```

- [ ] **Step 2: 追加 api.ts**

```ts
// 追加到 src/api.ts（imports 补 PageRow, LabelSummary）
export const ingestFolder = (srcDir: string) => invoke<number>("ingest_folder", { srcDir });
export const readImage = (filename: string) => invoke<number[]>("read_image", { filename });
export const listPages = () => invoke<PageRow[]>("list_pages");
export const setPageLabel = (pageId: number, studentId: number | null, problemNumber: number | null) =>
  invoke<void>("set_page_label", { pageId, studentId, problemNumber });
export const addStudent = (name: string, examNumber: string | null) =>
  invoke<number>("add_student", { name, examNumber });
export const labelingSummary = () => invoke<LabelSummary>("labeling_summary");
```

- [ ] **Step 3: 实现 useImage composable**

```ts
// src/composables/useImage.ts
import { ref, onUnmounted } from "vue";
import { readImage } from "../api";

// filename → blob URL；自动 revoke 上一张，卸载时清理
export function useImage() {
  const url = ref<string | null>(null);
  let current: string | null = null;
  async function show(filename: string | null) {
    if (current) { URL.revokeObjectURL(current); current = null; }
    url.value = null;
    if (!filename) return;
    const bytes = await readImage(filename);           // number[]
    const blob = new Blob([new Uint8Array(bytes)]);
    current = URL.createObjectURL(blob);
    url.value = current;
  }
  onUnmounted(() => { if (current) URL.revokeObjectURL(current); });
  return { url, show };
}
```

- [ ] **Step 4: 构建验证 + 提交**

Run: `npm run build`
Expected: 通过。

```bash
git add src/types.ts src/api.ts src/composables/useImage.ts
git commit -m "feat(front): ingest/label api + types + read_image→blob composable"
```

---

## Task 6: 纯 Label reducer `useLabelKeys`（Vitest 重测）

**Files:**
- Create: `src/composables/useLabelKeys.ts`, `src/composables/useLabelKeys.test.ts`

**Interfaces:**
- Produces:
  - `initialLabelState(): LabelState`
  - `reduceLabelKey(state, key, ctx): LabelResult`
  - `pickStudent(state, studentId): LabelResult`（花名册选人确认后，视图调用：当前页记为该生姓名页并前进）
  - 类型：`LabelState{ index, currentStudent: number|null, nextProblem, picker }`、`LabelCtx{ pageCount }`、`LabelEffect`（`{kind:'none'}` | `{kind:'openPicker'}` | `{kind:'assign'; studentId; problemNumber}`）、`LabelResult{ state, effect }`
  - 语义：`←/→` 只翻页(browse, none)；`S` 开选人；`Enter/空格` 给当前页派下一题号并前进(nextProblem++)；`C` 派 nextProblem-1(不进)并前进；`N` 跳题(nextProblem++，停在原页)；`currentStudent==null` 时 Enter/C/N 皆 none。`pickStudent`：当前页派 `(studentId, 0)`(姓名页)，`currentStudent=studentId, nextProblem=1, picker=false`，前进。换页/派题时 index 夹 `[0, pageCount-1]`。

- [ ] **Step 1: 写失败测试**

```ts
// src/composables/useLabelKeys.test.ts
import { describe, it, expect } from "vitest";
import { initialLabelState, reduceLabelKey, pickStudent } from "./useLabelKeys";
import type { LabelCtx } from "./useLabelKeys";

const ctx: LabelCtx = { pageCount: 6 };
const s0 = initialLabelState();

describe("Label reducer", () => {
  it("←/→ 只翻页，不派标", () => {
    expect(reduceLabelKey(s0, "ArrowRight", ctx).state.index).toBe(1);
    expect(reduceLabelKey(s0, "ArrowRight", ctx).effect).toEqual({ kind: "none" });
    expect(reduceLabelKey({ ...s0, index: 2 }, "ArrowLeft", ctx).state.index).toBe(1);
    // 夹边界
    expect(reduceLabelKey(s0, "ArrowLeft", ctx).state.index).toBe(0);
  });
  it("S 开花名册选人", () => {
    const r = reduceLabelKey(s0, "s", ctx);
    expect(r.effect).toEqual({ kind: "openPicker" });
    expect(r.state.picker).toBe(true);
  });
  it("选人 → 当前页记姓名页(题0)、设当前学生、nextProblem=1、前进", () => {
    const r = pickStudent({ ...s0, picker: true, index: 0 }, 42);
    expect(r.effect).toEqual({ kind: "assign", studentId: 42, problemNumber: 0 });
    expect(r.state.currentStudent).toBe(42);
    expect(r.state.nextProblem).toBe(1);
    expect(r.state.picker).toBe(false);
    expect(r.state.index).toBe(1); // 前进
  });
  it("Enter 顺序派题号并前进；无当前学生时不派", () => {
    expect(reduceLabelKey(s0, "Enter", ctx).effect).toEqual({ kind: "none" }); // 还没选学生
    let s = { ...s0, currentStudent: 42, nextProblem: 1, index: 1 };
    const r1 = reduceLabelKey(s, "Enter", ctx);
    expect(r1.effect).toEqual({ kind: "assign", studentId: 42, problemNumber: 1 });
    expect(r1.state.nextProblem).toBe(2);
    expect(r1.state.index).toBe(2);
  });
  it("C 接上一题(不进 counter)并前进", () => {
    const s = { ...s0, currentStudent: 42, nextProblem: 4, index: 3 };
    const r = reduceLabelKey(s, "c", ctx);
    expect(r.effect).toEqual({ kind: "assign", studentId: 42, problemNumber: 3 }); // nextProblem-1
    expect(r.state.nextProblem).toBe(4); // 不变
    expect(r.state.index).toBe(4);
  });
  it("N 跳题：counter 进、停在原页", () => {
    const s = { ...s0, currentStudent: 42, nextProblem: 2, index: 3 };
    const r = reduceLabelKey(s, "n", ctx);
    expect(r.effect).toEqual({ kind: "none" });
    expect(r.state.nextProblem).toBe(3); // 声明题2无页，counter 跳到3
    expect(r.state.index).toBe(3);        // 不动
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `npx vitest run src/composables/useLabelKeys.test.ts`
Expected: 全失败（模块未实现）。

- [ ] **Step 3: 实现 reducer**

```ts
// src/composables/useLabelKeys.ts
export interface LabelState { index: number; currentStudent: number | null; nextProblem: number; picker: boolean }
export interface LabelCtx { pageCount: number }
export type LabelEffect =
  | { kind: "none" }
  | { kind: "openPicker" }
  | { kind: "assign"; studentId: number; problemNumber: number };
export interface LabelResult { state: LabelState; effect: LabelEffect }

export function initialLabelState(): LabelState {
  return { index: 0, currentStudent: null, nextProblem: 1, picker: false };
}
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const none = (state: LabelState): LabelResult => ({ state, effect: { kind: "none" } });

export function reduceLabelKey(state: LabelState, key: string, ctx: LabelCtx): LabelResult {
  if (state.picker) { // 选人态：只认 Esc 关闭；选人由视图调 pickStudent
    if (key === "Escape") return none({ ...state, picker: false });
    return none(state);
  }
  if (key === "ArrowLeft")  return none({ ...state, index: clamp(state.index - 1, 0, ctx.pageCount - 1) });
  if (key === "ArrowRight") return none({ ...state, index: clamp(state.index + 1, 0, ctx.pageCount - 1) });
  if (key === "s" || key === "S") return { state: { ...state, picker: true }, effect: { kind: "openPicker" } };
  if (state.currentStudent == null) return none(state); // 未选学生，派题类键无效
  const sid = state.currentStudent;
  if (key === "Enter" || key === " ") {
    const problemNumber = state.nextProblem;
    return { state: { ...state, nextProblem: state.nextProblem + 1, index: clamp(state.index + 1, 0, ctx.pageCount - 1) },
             effect: { kind: "assign", studentId: sid, problemNumber } };
  }
  if (key === "c" || key === "C") {
    const problemNumber = Math.max(1, state.nextProblem - 1);
    return { state: { ...state, index: clamp(state.index + 1, 0, ctx.pageCount - 1) },
             effect: { kind: "assign", studentId: sid, problemNumber } };
  }
  if (key === "n" || key === "N") { // 跳题：counter 进、停在原页
    return none({ ...state, nextProblem: state.nextProblem + 1 });
  }
  return none(state);
}

// 视图在花名册选人确认后调用：当前页记为该生姓名页(题0)，设当前学生并前进
export function pickStudent(state: LabelState, studentId: number): LabelResult {
  return {
    state: { ...state, currentStudent: studentId, nextProblem: 1, picker: false, index: state.index + 1 },
    effect: { kind: "assign", studentId, problemNumber: 0 },
  };
}
```

- [ ] **Step 4: 跑测试确认通过 + 提交**

Run: `npx vitest run src/composables/useLabelKeys.test.ts`
Expected: 全 PASS（pristine）。

```bash
git add src/composables/useLabelKeys.ts src/composables/useLabelKeys.test.ts
git commit -m "feat(label): 纯 Label reducer + 全键位单测（S/Enter/C/N/翻页/选人）"
```

---

## Task 7: LabelView —— 单图浏览器 + 花名册选人 + 确认总表

**Files:**
- Create: `src/views/LabelView.vue`
- Modify: `src/App.vue`（加"标注"路由）

**Interfaces:**
- Consumes: `api`（listPages/setPageLabel/addStudent/labelingSummary/listStudents）、`useImage`、`useLabelKeys`。
- Produces: 标注界面——`←/→` 翻真图；`S` 弹选人(NModal，搜姓名/键考号，回车确认→`pickStudent`)；`Enter/C/N` 派题号(每步 `setPageLabel` 落库)；右侧状态栏(当前学生/题号进度/未标注数)；下方"确认总表"(读 `labeling_summary`，页数≠N 红标 count_ok=false、缺考单列)。

- [ ] **Step 1: 实现 LabelView.vue**

```vue
<!-- src/views/LabelView.vue -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { listPages, setPageLabel, labelingSummary, listStudents, addStudent } from "../api";
import type { PageRow, LabelSummary, Student } from "../types";
import { useImage } from "../composables/useImage";
import { initialLabelState, reduceLabelKey, pickStudent, type LabelState, type LabelEffect } from "../composables/useLabelKeys";
import { NModal, NInput, NButton, NAlert } from "naive-ui";

const pages = ref<PageRow[]>([]);
const students = ref<Student[]>([]);
const summary = ref<LabelSummary | null>(null);
const ls = ref<LabelState>(initialLabelState());
const { url, show } = useImage();
const errorMsg = ref("");
const pickQuery = ref("");

const cur = computed(() => pages.value[ls.value.index] ?? null);
const filteredStudents = computed(() => {
  const q = pickQuery.value.trim().toLowerCase();
  if (!q) return students.value;
  return students.value.filter(s => s.name.toLowerCase().includes(q) || (s.exam_number ?? "").toLowerCase().includes(q));
});

async function reload() {
  pages.value = await listPages();
  students.value = await listStudents();
  await refreshImage();
}
async function refreshImage() { await show(cur.value ? cur.value.image_path : null); }
async function refreshSummary() { summary.value = await labelingSummary(); }

async function applyEffect(eff: LabelEffect) {
  if (eff.kind === "assign" && cur.value) {
    await setPageLabel(cur.value.id, eff.studentId, eff.problemNumber);
    // 本地同步，避免整列重查
    cur.value.student_id = eff.studentId; cur.value.problem_number = eff.problemNumber; cur.value.status = "labeled";
  }
}
async function onKey(e: KeyboardEvent) {
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  if (ls.value.picker) return; // 选人态交给 NModal 输入
  const before = ls.value.index;
  const r = reduceLabelKey(ls.value, e.key, ctx.value);
  ls.value = r.state;
  e.preventDefault();
  await applyEffect(r.effect);
  if (ls.value.index !== before) await refreshImage();
}
const ctx = computed(() => ({ pageCount: pages.value.length }));

async function confirmPick(studentId: number) {
  const r = pickStudent(ls.value, studentId);
  ls.value = r.state;
  pickQuery.value = "";
  await applyEffect(r.effect);
  await refreshImage();
}
async function addAndPick() {
  const name = pickQuery.value.trim();
  if (!name) return;
  const id = await addStudent(name, null); // 临时新增
  students.value = await listStudents();
  await confirmPick(id);
}

onMounted(() => { window.addEventListener("keydown", onKey); reload(); });
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="label">
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" />
    <div v-if="!pages.length" class="empty">没有图片。先在"考试设置"导入图片文件夹。</div>
    <template v-else>
      <div class="pane">
        <div class="img">
          <img v-if="url" :src="url" alt="答卷" />
          <div v-else class="ph">（无图/加载中）</div>
        </div>
        <aside class="side">
          <p>第 {{ ls.index + 1 }} / {{ pages.length }} 张</p>
          <p>当前学生：{{ ls.currentStudent ?? "—" }}</p>
          <p>下一题号：{{ ls.nextProblem }}</p>
          <p v-if="cur">本页：{{ cur.problem_number === 0 ? "姓名页" : (cur.problem_number ?? "未标注") }}</p>
          <p class="keys">[S]姓名页/选人 [Enter]派题 [C]接上题 [N]跳题 [←→]翻页</p>
          <n-button size="small" @click="refreshSummary">刷新确认总表</n-button>
        </aside>
      </div>

      <!-- 花名册选人 -->
      <n-modal v-model:show="ls.picker" :close-on-esc="false" preset="card" title="选人（搜姓名/键考号）" style="width:420px">
        <n-input v-model:value="pickQuery" placeholder="姓名或考号" autofocus @keyup.enter="filteredStudents[0] && confirmPick(filteredStudents[0].id)" />
        <ul class="picklist">
          <li v-for="s in filteredStudents" :key="s.id" @click="confirmPick(s.id)">{{ s.name }}（{{ s.exam_number ?? "—" }}）</li>
        </ul>
        <n-button size="small" @click="addAndPick">＋ 名册没有，临时新增「{{ pickQuery }}」</n-button>
      </n-modal>

      <!-- 确认总表 -->
      <div v-if="summary" class="summary">
        <h3>确认总表（未标注 {{ summary.unlabeled_pages }} 张）</h3>
        <table>
          <thead><tr><th>学生</th><th>答题页</th><th>题数N</th><th>校验</th></tr></thead>
          <tbody>
            <tr v-for="st in summary.stacks" :key="st.student_id" :class="{ bad: !st.count_ok }">
              <td>{{ st.student_name }}</td><td>{{ st.answer_pages }}</td><td>{{ st.problem_count }}</td>
              <td>{{ st.count_ok ? "✓" : "✗ 页数不符，逐页指认" }}</td>
            </tr>
          </tbody>
        </table>
        <p v-if="summary.absent_students.length" class="absent">
          缺考（花名册有、无卷）：{{ summary.absent_students.map(s => s.name).join("、") }}
        </p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.label { height: 100vh; display: flex; flex-direction: column; font-family: ui-monospace, monospace; color: #d0d0d0; }
.pane { flex: 1; display: flex; min-height: 0; }
.img { flex: 1; display: flex; align-items: center; justify-content: center; overflow: auto; }
.img img { max-width: 100%; max-height: 100%; }
.ph { border: 1px dashed #555; padding: 40px; color: #888; }
.side { width: 260px; border-left: 1px solid #333; padding: 12px; }
.side .keys { color: #888; font-size: 12px; margin-top: 12px; }
.picklist { max-height: 220px; overflow: auto; margin: 8px 0; }
.picklist li { cursor: pointer; padding: 2px 4px; list-style: none; }
.picklist li:hover { color: #7fd; }
.summary { border-top: 1px solid #333; padding: 8px 12px; max-height: 30vh; overflow: auto; }
.summary table { border-collapse: collapse; }
.summary th, .summary td { border: 1px solid #444; padding: 2px 10px; }
.summary tr.bad td { color: #f77; }
.summary .absent { color: #fb7; }
</style>
```

- [ ] **Step 2: App.vue 加"标注"路由**

```ts
// route 类型加 "label"
const route = ref<"setup" | "label" | "grade" | "export">("setup");
```
```html
<!-- nav 追加（放在 考试设置 与 判分 之间） -->
<button @click="route = 'label'">标注</button>
<!-- 视图区追加 -->
<LabelView v-else-if="route === 'label'" />
```
并 `import LabelView from "./views/LabelView.vue";`。

- [ ] **Step 3: 构建验证 + 提交**

Run: `npm run build`
Expected: 通过。

```bash
git add src/views/LabelView.vue src/App.vue
git commit -m "feat(label): LabelView——真图浏览器/花名册选人/确认总表(§7.0/缺考)"
```

---

## Task 8: Ingest 入口 + GradeView 选题 + 真图

**Files:**
- Modify: `src/views/SetupView.vue`（导入图片文件夹入口）、`src/views/GradeView.vue`（选题 + 真图）

**Interfaces:**
- Consumes: `ingestFolder`、`listProblems`、`buildQueue`、`useImage`、`@tauri-apps/plugin-dialog` `open`。

- [ ] **Step 1: SetupView 加导入入口**

在 `src/views/SetupView.vue` 的按钮区加"导入图片文件夹"，选目录后调 `ingestFolder`：

```ts
// SetupView.vue <script setup> 追加
import { ingestFolder } from "../api";
const ingestMsg = ref("");
async function doIngest() {
  ingestMsg.value = "";
  try {
    const dir = await open({ directory: true, multiple: false, title: "选择图片文件夹" });
    if (typeof dir !== "string") return;
    const n = await ingestFolder(dir);
    ingestMsg.value = `已导入 ${n} 张图（去"标注"页开始标注）`;
  } catch (e) { errorMsg.value = String(e); }
}
```
```html
<!-- 模板按钮区追加（需已打开考试）-->
<n-button v-if="exam" @click="doIngest">导入图片文件夹…</n-button>
<n-alert v-if="ingestMsg" type="success" :title="ingestMsg" closable @close="ingestMsg=''" />
```

- [ ] **Step 2: GradeView 选题 + 真图**

改 `src/views/GradeView.vue`：把固定 `problemNumber=1` 换成用户可选；左图用 `useImage`。

```ts
// GradeView.vue <script setup> 关键改动
import { listProblems } from "../api";
import { useImage } from "../composables/useImage";
import type { Problem } from "../types";

const problems = ref<Problem[]>([]);
const problemNumber = ref<number | null>(null);
const { url: imgUrl, show: showImg } = useImage();

async function initProblems() {
  problems.value = await listProblems();
  if (problems.value.length && problemNumber.value == null) problemNumber.value = problems.value[0].number;
}
async function loadQueue() {
  try {
    if (problemNumber.value == null) { queue.value = []; return; }
    queue.value = await buildQueue(problemNumber.value);
    presets.value = current.value ? await listPresets(current.value.problem_id) : [];
    await refreshPeek();
    await refreshImg();
  } catch (e) { errorMsg.value = String(e); }
}
async function refreshImg() {
  // 真图：当前速览页或当前单元首图；fake:// 或缺失走占位
  const path = shownImage.value;
  await showImg(path && !path.startsWith("fake://") ? path : null);
}
// 选题变化 → 重载队列
function onSelectProblem(n: number) { problemNumber.value = n; ls.value = initialGradeState(); loadQueue(); }

onMounted(async () => { window.addEventListener("keydown", onKey); await initProblems(); await loadQueue(); });
```
```html
<!-- 顶部加题目选择器 -->
<header class="picker">
  题目：
  <button v-for="p in problems" :key="p.id" :class="{on: p.number===problemNumber}" @click="onSelectProblem(p.number)">
    题{{ p.number }}
  </button>
</header>
<!-- 左图区：真图优先，缺图降级占位 -->
<img v-if="imgUrl" :src="imgUrl" alt="答卷" />
<div v-else class="placeholder">…（保留现有占位块，显示学生名·题号·速览偏移）…</div>
```
> 注：`shownImage` 现返回文件名(相对)；真图经 `useImage`→`read_image` 显示。溢出多页与轴2 速览沿用现有 `shownImage` 计算，只把渲染从直接 `:src` 换成 `useImage`。速览偏移变化时调用 `refreshImg()`。

- [ ] **Step 3: 构建验证 + 提交**

Run: `npm run build`
Expected: 通过。

```bash
git add src/views/SetupView.vue src/views/GradeView.vue
git commit -m "feat: Setup 导入图片文件夹入口 + GradeView 选题与真图显示"
```

---

## Task 9: 集成验证与收尾

**Files:** 无新增

- [ ] **Step 1: 全量测试**

Run: `cargo test && npx vitest run`
Expected: Rust 全绿（含 ingest/label 新测）、前端 reducer 全绿（含 useLabelKeys）。

- [ ] **Step 2: 端到端验收（CDP，同前）**

`npm run tauri dev`：
1. 新建/打开一个考试；在"考试设置"点"导入图片文件夹…"选一个含真实图的目录 → 提示导入 N 张；确认这些图拷进了该考试 `images/`。
2. "标注"页：`←/→` 翻真图；在姓名页 `S` → 选人；`Enter` 顺序派题；造一个溢出(`C`)、一个跳题(`N`)、留一个花名册学生不标(缺考)。
3. "刷新确认总表"：页数≠N 的学生红标、缺考单列、未标注计数正确。
4. "判分"页：题目选择器切到题2 → 载该题队列、左侧显示真图、批几份。
5. "导出"：覆盖率/CSV 正常。
> **注意**：导入用的是原生目录对话框，CDP 难驱动——如无法自动驱动，则用 `ingest_folder` 直传一个测试目录路径验证后端链路，并明确报告"目录选择器需人工点一次"、其余 UI 交互 CDP 驱动。据实报告。

- [ ] **Step 3: 收尾 commit + tag**

```bash
git commit -am "chore: Ingest+Label+真图/选题 完成——可批真实扫描卷" --allow-empty
git tag m2m4-ingest-label
```

---

## Self-Review（对照实现设计 spec 的覆盖）

- **Ingest**：Task 1（排序/建行）+ Task 4（枚举/拷贝/追加/相对文件名）✓；spec §1
- **真图 read_image→blob，路径限定 images/**：Task 4（越界拒绝）+ Task 5（useImage）✓；spec §2
- **Label reducer S/Enter/C/N/翻页/选人 + 单测**：Task 6 ✓；spec §3.2
- **§7.0 页数≠N 强制指认（count_ok）**：Task 3 + Task 7 红标 ✓；spec §3.3
- **增量写库**：Task 7 每步 setPageLabel ✓；spec §3.4
- **确认总表 + 缺考 + 待匹配/临时加**：Task 3（summary）+ Task 7（表/临时新增）✓；spec §3.5
- **判分选题 + 真图**：Task 8 ✓；spec §4
- **零 schema 改动**：全程只加代码 ✓；spec §5
- **测试策略（Rust/Vitest/CDP，capability 路径标注人工）**：Task 9 ✓；spec §7

**已知的、有意的后续缺口（非占位）**：二维码/OCR 自动识别、成像段自有化、asset 协议按需加载大图、题内步级诊断——均属 v0.2 §6/§7.4 明列的后续，非本片。
