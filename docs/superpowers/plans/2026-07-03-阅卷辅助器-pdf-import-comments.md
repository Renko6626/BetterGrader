# 阅卷辅助器 · PDF 一键导入 + 每题评语 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 指着一个"每个 PDF 以人名命名、内容是这人整份卷"的文件夹，一键把 PDF 拆成图片并自动标注好，直接开批；并给每道题加可选评语，导出时可选把评语列并入 CSV。

**Architecture:** PDF 在**前端用 pdf.js 渲染**成每页 PNG（page1=姓名页、page k=第 k−1 题），字节交 Rust 存进考试 `images/` 并**建好已标注的 page 行**——于是判分/导出/确认总表(§7.0)/缺考全部原样复用。评语是 `score` 表新增一列 `comment`，判分时录入、导出时可选并入。

**Tech Stack:** 沿用(Tauri v2 · rusqlite · Vue3+TS+Vite · Vitest · naive-ui · plugin-dialog) + 新增前端 `pdfjs-dist`。

## Global Constraints

- **PDF 转图片走前端 pdfjs-dist**（不附带原生二进制）；每页渲染成 PNG 字节交 Rust。
- **自动标注**：每个 PDF = 一个学生(名字=文件名去扩展名)；`page_index 0 = 姓名页(problem_number=0)`，`page_index k = 第 k 题(problem_number=k)`。→ `problem_number = page_index`。
- **复用现有全流程**：存进 `images/`、建 `page(status='labeled')` 行后，Grade/Export/确认总表/缺考**一行不改**。页数≠N+1 的在确认总表标红、手动修（复用 §7.0）。
- **评语**：`score` 表加 `comment TEXT`（幂等迁移，兼容既有库）；每(学生,题)一条；判分录入；导出可选并入。
- **CSV 评语布局**：`所有分在前、所有评语在后` —— `姓名,考号,题1..题N,总分,[排名],题1评语..题N评语`（仅当勾选"含评语"）。未判/存疑/缺考的评语留空。
- `image_path` 仍存**文件名**(相对 images/)；数据不出本地；AI 永不写 score.total。
- **构建**：`npm run build`/`vite build` 在本环境卡死——**任何前端任务都不要跑它**；写完代码即可，完整构建由用户手动做。`cargo test`/`cargo build`/`npx vitest run` 正常可用。

---

## File Structure

```
crates/grading-core/src/
  db.rs        # 加幂等迁移：score.comment 列
  schema.sql   # CREATE score 加 comment 列（新库）
  label.rs     # 加 add_labeled_page（PDF 自动标注建行）
  grading.rs   # 加 set_comment；build_export 带评语
  export.rs    # export_to_csv 加 include_comments（分在前评语在后）
  models.rs    # Cell 加 comment 字段
src-tauri/src/
  commands.rs  # list_pdfs / read_pdf / save_pdf_page / set_comment / rename_student / delete_student
  main.rs      # 注册
src/
  api.ts / types.ts        # 同步
  composables/usePdf.ts     # 【新】pdf.js: bytes → PNG[]
  views/SetupView.vue       # 加"导入 PDF 文件夹"入口 + 学生改名/删除
  views/GradeView.vue       # 加评语输入
  views/ExportView.vue      # 加"含评语"开关
package.json                # + pdfjs-dist
```

**责任边界**：`grading-core` 仍纯（迁移/建行/评语/导出全可 `cargo test`）；PDF 渲染在前端（pdf.js），Rust 只存字节 + 建行；评语是 score 的一个字段，走既有 with_exam 通道。

---

## Task 1: score.comment 列 —— schema + 幂等迁移

**Files:**
- Modify: `crates/grading-core/src/schema.sql`, `crates/grading-core/src/db.rs`

**Interfaces:**
- Produces: 新库的 `score` 表含 `comment TEXT`；既有库经 `Db::open` 自动补上 `comment` 列（幂等）。

- [ ] **Step 1: schema 加列（新库）**

在 `crates/grading-core/src/schema.sql` 的 `score` 表定义里，`preset_id INTEGER,` 之后加 `comment TEXT,`：
```sql
CREATE TABLE IF NOT EXISTS score (
  id INTEGER PRIMARY KEY,
  student_id INTEGER NOT NULL, problem_id INTEGER NOT NULL,
  total INTEGER, state TEXT NOT NULL DEFAULT 'Ungraded',
  preset_id INTEGER, comment TEXT, grader TEXT, submitted_at TEXT,
  UNIQUE(student_id, problem_id)
);
```

- [ ] **Step 2: 写失败测试**

```rust
// 追加到 crates/grading-core/src/db.rs 的 tests 模块
#[test]
fn score_has_comment_column_and_migration_is_idempotent() {
    let db = Db::open_in_memory().unwrap();
    // 列存在
    let has: i64 = db.conn.query_row(
        "SELECT count(*) FROM pragma_table_info('score') WHERE name='comment'", [], |r| r.get(0)).unwrap();
    assert_eq!(has, 1);
    // 再跑一次迁移不报错（幂等）
    super::migrate(&db.conn).unwrap();
    let has2: i64 = db.conn.query_row(
        "SELECT count(*) FROM pragma_table_info('score') WHERE name='comment'", [], |r| r.get(0)).unwrap();
    assert_eq!(has2, 1);
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core score_has_comment`
Expected: 编译失败——`migrate` 未定义。

- [ ] **Step 4: 实现幂等迁移（db.rs）**

在 `crates/grading-core/src/db.rs` 的 `Db::init` 里，`execute_batch(SCHEMA)` 之后调用 `migrate`；新增 `pub(crate) fn migrate`：
```rust
// db.rs：init 内 schema 应用后加一行
Self::migrate_conn(&conn)?;

// 新增（pub(crate) 便于测试）
pub(crate) fn migrate(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    // 既有库补 score.comment 列（新库已由 schema 建好，pragma 检测避免重复 ALTER）
    let has: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('score') WHERE name='comment'", [], |r| r.get(0))?;
    if has == 0 {
        conn.execute("ALTER TABLE score ADD COLUMN comment TEXT", [])?;
    }
    Ok(())
}
```
> 注意：把 `Db::init` 里调用写成 `Self::migrate(&conn)?;`（与测试里 `super::migrate` 对应，函数名统一为 `migrate`）。若 init 是关联函数，用 `Db::migrate(&conn)?;`。保持测试与实现同名 `migrate`。

- [ ] **Step 5: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core`
Expected: 全绿（含新测）。

```bash
git add crates/grading-core/src/schema.sql crates/grading-core/src/db.rs
git commit -m "feat(core): score.comment 列 + 幂等迁移（兼容既有库）"
```

---

## Task 2: grading-core add_labeled_page —— PDF 自动标注建行

**Files:**
- Modify: `crates/grading-core/src/label.rs`

**Interfaces:**
- Consumes: `Db`、`ingest::next_seq`
- Produces: `add_labeled_page(db, exam_id, student_id, problem_number, filename, seq) -> anyhow::Result<i64>`（插一条**已标注** page：给定 student_id/problem_number，status='labeled'，返回 page id）

- [ ] **Step 1: 写失败测试**

```rust
// 追加到 crates/grading-core/src/label.rs 的 tests 模块
#[test]
fn add_labeled_page_inserts_labeled_row() {
    use crate::setup::create_exam;
    let db = Db::open_in_memory().unwrap();
    let exam = create_exam(&db, "E", "2026-07-03").unwrap();
    let sid = add_student(&db, exam, "张三", None).unwrap();
    let id = add_labeled_page(&db, exam, sid, 0, "s_0.png", 0).unwrap(); // 姓名页
    add_labeled_page(&db, exam, sid, 1, "s_1.png", 1).unwrap();          // 第1题
    let pages = list_pages(&db, exam).unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!((pages[0].student_id, pages[0].problem_number, pages[0].status.as_str()),
               (Some(sid), Some(0), "labeled"));
    assert_eq!(pages[1].problem_number, Some(1));
    let _ = id;
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p grading-core add_labeled_page`
Expected: 编译失败——未定义。

- [ ] **Step 3: 实现 add_labeled_page**

```rust
// 追加到 crates/grading-core/src/label.rs（非 tests）
pub fn add_labeled_page(db: &Db, exam_id: i64, student_id: i64, problem_number: i64, filename: &str, seq: i64) -> Result<i64> {
    db.conn.execute(
        "INSERT INTO page(exam_id, student_id, problem_number, image_path, seq, status)
         VALUES(?1, ?2, ?3, ?4, ?5, 'labeled')",
        (exam_id, student_id, problem_number, filename, seq))?;
    Ok(db.conn.last_insert_rowid())
}
```

- [ ] **Step 4: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core label`
Expected: PASS。

```bash
git add crates/grading-core/src/label.rs
git commit -m "feat(core): add_labeled_page —— PDF 自动标注建行"
```

---

## Task 3: grading-core 评语 —— set_comment + 导出带评语 + CSV 布局

**Files:**
- Modify: `crates/grading-core/src/grading.rs`, `crates/grading-core/src/export.rs`, `crates/grading-core/src/models.rs`

**Interfaces:**
- Consumes: `Db`、`ScoreState`、`build_export`
- Produces:
  - `grading::set_comment(db, student_id, problem_id, comment: &str) -> Result<()>`（upsert：score 行不存在则建 Ungraded 行只带评语；存在则更新 comment）
  - `Cell` 加字段 `comment: Option<String>`；`build_export` 填每单元评语
  - `export_to_csv(data, include_comments: bool)`（新增第二参）：true 时在 `总分[/排名]` 之后追加 `题1评语..题N评语` 列；单元评语按状态取，缺考/未标注留空

- [ ] **Step 1: Cell 加 comment（models.rs）**

```rust
// crates/grading-core/src/models.rs：Cell 结构加 comment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cell { pub total: Option<i64>, pub state: String, pub comment: Option<String> }
```

- [ ] **Step 2: 写失败测试**

```rust
// 追加到 crates/grading-core/src/grading.rs 的 tests 模块
#[test]
fn set_comment_upserts_without_clobbering_score() {
    use crate::{setup::*, ScoreState};
    let db = Db::open_in_memory().unwrap();
    let exam = create_exam(&db, "E", "2026-07-03").unwrap();
    let p1 = add_problem(&db, exam, 1, "一", 10).unwrap();
    import_roster(&db, exam, &[crate::RosterRow{name:"甲".into(), exam_number:None}]).unwrap();
    let sid = list_students(&db, exam).unwrap()[0].id;
    // 先评语（无分）→ 建 Ungraded 行带评语
    set_comment(&db, sid, p1, "步骤不全").unwrap();
    // 再给分 → 不冲掉评语
    set_score(&db, sid, p1, Some(7), None, ScoreState::Graded).unwrap();
    let (total, comment): (Option<i64>, Option<String>) = db.conn.query_row(
        "SELECT total, comment FROM score WHERE student_id=?1 AND problem_id=?2",
        (sid, p1), |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(total, Some(7));
    assert_eq!(comment.as_deref(), Some("步骤不全"));
}
```
```rust
// 追加到 crates/grading-core/src/export.rs 的 tests 模块
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
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core set_comment` 与 `cargo test -p grading-core csv_appends`
Expected: 编译失败——`set_comment` 未定义、`export_to_csv` 参数不符、`Cell.comment` 缺失。

- [ ] **Step 4: 实现**

`grading.rs`：
```rust
pub fn set_comment(db: &Db, student_id: i64, problem_id: i64, comment: &str) -> Result<()> {
    db.conn.execute(
        "INSERT INTO score(student_id, problem_id, state, comment)
         VALUES(?1, ?2, 'Ungraded', ?3)
         ON CONFLICT(student_id, problem_id) DO UPDATE SET comment=?3",
        (student_id, problem_id, comment))?;
    Ok(())
}
```
`export.rs` build_export：读 score 时一并取 comment，填进 Cell。把现有 `SELECT total, state ...` 改为 `SELECT total, state, comment ...`，match 分支填 `Cell{ total, state, comment }`；`Ungraded`/`Absent` 单元 `comment` 取库值（未判也可能有评语）——即缺省 None，但若库里该行有 comment 则带上。缺考单元 comment=None。
```rust
// build_export 内，score 查询：
let row = db.conn.query_row(
    "SELECT total, state, comment FROM score WHERE student_id=?1 AND problem_id=?2",
    (stu.id, p.id),
    |r| Ok((r.get::<_,Option<i64>>(0)?, r.get::<_,String>(1)?, r.get::<_,Option<String>>(2)?)),
).optional()?;
let (total, state, comment) = match row {
    Some((t, s, c)) => (t, ScoreState::from_str(&s), c),
    None => (None, ScoreState::Ungraded, None),
};
// ... 现有 total=None(Ungraded 强制) 逻辑保留 ...
cells.push(Cell { total: cell_total, state: state.as_str().into(), comment });
// 缺考分支：cells.push(Cell{ total:None, state:"Absent".into(), comment:None });
```
`export.rs` export_to_csv：加第二参 `include_comments: bool`；表头在 `总分[,排名]` 之后追加 `题{n}评语`；每行在末尾追加各题 comment（缺考行评语列留空）：
```rust
pub fn export_to_csv(data: &ExportData, include_comments: bool) -> String {
    // ...现有表头构造后：
    if include_comments {
        for n in &data.problem_numbers { header.push(format!("题{n}评语")); }
    }
    // ...现有每行字段构造后（追加评语）：
    if include_comments {
        for c in &r.cells {
            let text = if r.absent { String::new() } else { c.comment.clone().unwrap_or_default() };
            f.push(csv_field(&text));
        }
    }
    // ...
}
```
> 现有 export_to_csv 的所有调用点（Task 5 IPC 里的 save_csv）改为传第二参。旧测试 `export_to_csv(&data)` 改为 `export_to_csv(&data, false)`。

- [ ] **Step 5: 跑测试确认通过 + 提交**

Run: `cargo test -p grading-core`
Expected: 全绿（含新评语测 + 既有导出测已改两参）。

```bash
git add crates/grading-core/src/grading.rs crates/grading-core/src/export.rs crates/grading-core/src/models.rs
git commit -m "feat(core): set_comment(upsert) + 导出带评语 + CSV 含评语列(分前评语后)"
```

---

## Task 4: Rust 命令 —— PDF 导入 + 评语 + 学生编辑

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Produces（命令）:
  - `list_pdfs(dir: String) -> Vec<String>`（dir 下 `.pdf` 文件名，自然排序）
  - `read_pdf(dir: String, filename: String) -> Vec<u8>`（读 dir/filename 字节；filename 防遍历）
  - `save_pdf_page(student_id: i64, page_index: i64, filename: String, bytes: Vec<u8>) -> ()`（把 PNG 存进当前考试 images/filename；建 labeled page：problem_number=page_index，seq=next_seq）
  - `set_comment(student_id, problem_id, comment: String) -> ()`
  - `rename_student(student_id, name: String) -> ()`、`delete_student(student_id) -> ()`（删学生及其 page/score）

- [ ] **Step 1: 实现命令（commands.rs 追加）**

```rust
use grading_core::ingest;
// list_pdfs
#[tauri::command]
pub fn list_pdfs(_state: tauri::State<AppState>, dir: String) -> R<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(e)? {
        let entry = entry.map_err(e)?;
        if !entry.file_type().map_err(e)?.is_file() { continue; }
        let n = entry.file_name().to_string_lossy().to_string();
        if n.rsplit('.').next().unwrap_or("").eq_ignore_ascii_case("pdf") { names.push(n); }
    }
    Ok(ingest::sort_scan_order(names))
}
#[tauri::command]
pub fn read_pdf(_state: tauri::State<AppState>, dir: String, filename: String) -> R<Vec<u8>> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") { return Err("非法文件名".into()); }
    std::fs::read(std::path::Path::new(&dir).join(&filename)).map_err(e)
}
#[tauri::command]
pub fn save_pdf_page(state: tauri::State<AppState>, student_id: i64, page_index: i64, filename: String, bytes: Vec<u8>) -> R<()> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") { return Err("非法文件名".into()); }
    let guard = state.0.lock().map_err(e)?;
    let oe = guard.as_ref().ok_or_else(|| "no exam open".to_string())?;
    let images = oe.dir.join("images");
    std::fs::create_dir_all(&images).map_err(e)?;
    std::fs::write(images.join(&filename), &bytes).map_err(e)?;
    let seq = ingest::next_seq(&oe.db, oe.exam_id).map_err(e)?;
    grading_core::label::add_labeled_page(&oe.db, oe.exam_id, student_id, page_index, &filename, seq).map_err(e)?;
    Ok(())
}
#[tauri::command]
pub fn set_comment(state: tauri::State<AppState>, student_id: i64, problem_id: i64, comment: String) -> R<()> {
    with_exam(&state, |oe| grading::set_comment(&oe.db, student_id, problem_id, &comment))
}
#[tauri::command]
pub fn rename_student(state: tauri::State<AppState>, student_id: i64, name: String) -> R<()> {
    with_exam(&state, |oe| { oe.db.conn.execute("UPDATE student SET name=?2 WHERE id=?1", (student_id, &name))?; Ok(()) })
}
#[tauri::command]
pub fn delete_student(state: tauri::State<AppState>, student_id: i64) -> R<()> {
    with_exam(&state, |oe| {
        oe.db.conn.execute("DELETE FROM page WHERE student_id=?1", [student_id])?;
        oe.db.conn.execute("DELETE FROM score WHERE student_id=?1", [student_id])?;
        oe.db.conn.execute("DELETE FROM student WHERE id=?1", [student_id])?;
        Ok(())
    })
}
```

- [ ] **Step 2: 注册（main.rs generate_handler! 追加）**

```rust
            , commands::list_pdfs, commands::read_pdf, commands::save_pdf_page,
              commands::set_comment, commands::rename_student, commands::delete_student
```

- [ ] **Step 3: 编译验证（cargo，前台）+ 提交**

Run: `cargo build`（前台，等结束；不要跑 npm/tauri dev）
Expected: 成功。

```bash
git add src-tauri/
git commit -m "feat(ipc): PDF 导入(list_pdfs/read_pdf/save_pdf_page) + set_comment + 学生改名/删除"
```

---

## Task 5: 前端 api/types + pdf.js composable

**Files:**
- Modify: `src/types.ts`, `src/api.ts`, `package.json`
- Create: `src/composables/usePdf.ts`

**Interfaces:**
- Produces: 封装 `listPdfs/readPdf/savePdfPage/setComment/renameStudent/deleteStudent`；`Cell` TS 类型加 `comment`；`saveCsv` 加 `includeComments` 参数；`usePdf().renderToPngs(bytes) -> Uint8Array[]`。

- [ ] **Step 1: 装 pdfjs-dist**

Run: `npm install pdfjs-dist@^4`
`package.json` dependencies 出现 `pdfjs-dist`。

- [ ] **Step 2: types.ts 改 Cell + api.ts 封装**

```ts
// src/types.ts：Cell 加 comment
export interface Cell { total: number | null; state: "Graded"|"Flagged"|"Ungraded"|"Absent"; comment: string | null }
```
```ts
// src/api.ts 追加/改
export const listPdfs = (dir: string) => invoke<string[]>("list_pdfs", { dir });
export const readPdf = (dir: string, filename: string) => invoke<number[]>("read_pdf", { dir, filename });
export const savePdfPage = (studentId: number, pageIndex: number, filename: string, bytes: number[]) =>
  invoke<void>("save_pdf_page", { studentId, pageIndex, filename, bytes });
export const setComment = (studentId: number, problemId: number, comment: string) =>
  invoke<void>("set_comment", { studentId, problemId, comment });
export const renameStudent = (studentId: number, name: string) => invoke<void>("rename_student", { studentId, name });
export const deleteStudent = (studentId: number) => invoke<void>("delete_student", { studentId });
// saveCsv 加 includeComments
export const saveCsv = (path: string, includeComments: boolean) =>
  invoke<void>("save_csv", { path, includeComments });
```
> `save_csv` 的 Rust 命令需同步加 `include_comments: bool` 参并传给 `export_to_csv`（Task 4 若未改，在此任务的 Rust 侧补：`save_csv(path, include_comments)` → `export_to_csv(&data, include_comments)`）。**注意**：这要求改 `src-tauri` 的 save_csv 命令签名——放在 Task 4 一并改更顺；此处标注确保不漏。

- [ ] **Step 3: usePdf composable（pdf.js 渲染）**

```ts
// src/composables/usePdf.ts
import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export function usePdf() {
  // PDF 字节 → 每页 PNG 字节数组
  async function renderToPngs(bytes: Uint8Array, scale = 1.6): Promise<Uint8Array[]> {
    const pdf = await pdfjs.getDocument({ data: bytes }).promise;
    const out: Uint8Array[] = [];
    for (let i = 1; i <= pdf.numPages; i++) {
      const page = await pdf.getPage(i);
      const viewport = page.getViewport({ scale });
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      const ctx = canvas.getContext("2d")!;
      await page.render({ canvasContext: ctx, viewport }).promise;
      const blob: Blob = await new Promise((r) => canvas.toBlob((b) => r(b!), "image/png"));
      out.push(new Uint8Array(await blob.arrayBuffer()));
      canvas.width = 0; canvas.height = 0; // 释放
    }
    return out;
  }
  return { renderToPngs };
}
```

- [ ] **Step 4: 提交（不跑 npm build——由用户手动构建验证）**

```bash
git add src/types.ts src/api.ts src/composables/usePdf.ts package.json package-lock.json
git commit -m "feat(front): pdf.js composable + PDF/评语/学生编辑 api + Cell.comment"
```
> 本任务**不跑 `npm run build`**（环境卡死）。类型正确性靠 review；完整构建由用户手动做。

---

## Task 6: 前端 PDF 一键导入流程（SetupView）+ 学生编辑

**Files:**
- Modify: `src/views/SetupView.vue`

**Interfaces:**
- Consumes: `open`(dialog)、`listPdfs/readPdf/savePdfPage/addStudent/renameStudent/deleteStudent/listStudents`、`usePdf`。
- Produces: "导入 PDF 文件夹"入口（选目录→逐 PDF 渲染→自动建学生与已标注页）；花名册区可改名/删学生。

- [ ] **Step 1: SetupView 加 PDF 导入 + 学生编辑**

```ts
// SetupView.vue <script setup> 追加
import { open } from "@tauri-apps/plugin-dialog";
import { listPdfs, readPdf, savePdfPage, addStudent, renameStudent, deleteStudent } from "../api";
import { usePdf } from "../composables/usePdf";
const { renderToPngs } = usePdf();
const pdfMsg = ref("");
const importing = ref(false);
async function doImportPdfs() {
  pdfMsg.value = ""; errorMsg.value = "";
  try {
    const dir = await open({ directory: true, multiple: false, title: "选择 PDF 文件夹" });
    if (typeof dir !== "string") return;
    importing.value = true;
    const pdfs = await listPdfs(dir);
    let done = 0;
    for (const name of pdfs) {
      const studentName = name.replace(/\.pdf$/i, "");
      const sid = await addStudent(studentName, null);
      const bytes = new Uint8Array(await readPdf(dir, name));
      const pngs = await renderToPngs(bytes);
      for (let idx = 0; idx < pngs.length; idx++) {
        // page_index 0 = 姓名页(题0)，idx = 题号；problem_number = idx
        await savePdfPage(sid, idx, `${sid}_${idx}.png`, Array.from(pngs[idx]));
      }
      done++;
      pdfMsg.value = `已导入 ${done}/${pdfs.length} 份 PDF…`;
    }
    pdfMsg.value = `完成：导入 ${done} 份 PDF（去"判分"直接开批；页数不符的在"标注"确认总表里修）`;
    await refresh();
  } catch (e) { errorMsg.value = String(e); } finally { importing.value = false; }
}
async function doRename(sid: number, cur: string) {
  const name = window.prompt("改名", cur);
  if (name && name.trim()) { await renameStudent(sid, name.trim()); await refresh(); }
}
async function doDelete(sid: number) {
  if (window.confirm("删除该学生及其所有页/分？")) { await deleteStudent(sid); await refresh(); }
}
```
```html
<!-- 模板：导入区（需已打开考试）追加 -->
<n-button v-if="exam" :loading="importing" @click="doImportPdfs">导入 PDF 文件夹…</n-button>
<n-alert v-if="pdfMsg" type="success" :title="pdfMsg" closable @close="pdfMsg=''" />
<!-- 花名册区：每行加 改名/删除 -->
<!-- 若用 n-data-table，加一列 render 按钮；或最简用 ul 列表： -->
<ul v-if="students.length">
  <li v-for="s in students" :key="s.id">
    {{ s.name }}（{{ s.exam_number ?? "—" }}）
    <n-button size="tiny" @click="doRename(s.id, s.name)">改名</n-button>
    <n-button size="tiny" @click="doDelete(s.id)">删除</n-button>
  </li>
</ul>
```

- [ ] **Step 2: 提交（不跑 npm build）**

```bash
git add src/views/SetupView.vue
git commit -m "feat(front): 一键导入 PDF 文件夹(转图自动标注) + 学生改名/删除"
```

---

## Task 7: GradeView 评语输入 + ExportView 含评语开关

**Files:**
- Modify: `src/views/GradeView.vue`, `src/views/ExportView.vue`

**Interfaces:**
- Consumes: `setComment`（GradeView）、`saveCsv(path, includeComments)`（ExportView）。

- [ ] **Step 1: GradeView 评语输入**

在判分右侧面板加一个评语输入框，绑当前单元；失焦或按键提交 `setComment`。关键：评语输入框获焦时，键盘事件**不**进 `useGradeKeys`（避免打字被当判分键）。

```ts
// GradeView.vue <script setup> 追加
import { setComment } from "../api";
const commentText = ref("");
const commentFocused = ref(false);
// 载入/换单元时同步评语显示
function syncComment() { commentText.value = current.value?.comment ?? ""; }
async function saveCurrentComment() {
  if (!current.value) return;
  try { await setComment(current.value.student_id, current.value.problem_id, commentText.value);
        (current.value as any).comment = commentText.value; }
  catch (e) { errorMsg.value = String(e); }
}
```
> `GradingUnit`/`Cell` 需带 `comment`——`build_queue` 已经不返回 comment，需在 `GradingUnit` 里加 `comment: Option<String>` 并让 `build_queue` 查出来。**补**：Task 3 只改了 Export 的 Cell；这里判分要显示/回填评语，需 `grading::build_queue` 也带 comment。在 Task 3 或此任务的 Rust 侧给 `GradingUnit` 加 `comment` 字段并在 `build_queue` 的 score 查询里带上（`SELECT total, state, preset_id, comment ...`）。types.ts 的 `GradingUnit` 同步加 `comment: string | null`。
```ts
// onKey 开头：评语框获焦时不拦截判分键
function onKey(e: KeyboardEvent) {
  if (commentFocused.value) return; // 打字时放行给输入框
  // ...原有逻辑...
}
```
```html
<!-- 右侧面板底部加评语框 -->
<div class="comment">
  <textarea v-model="commentText" placeholder="本题评语（可选）"
    @focus="commentFocused = true" @blur="commentFocused = false; saveCurrentComment()"></textarea>
</div>
```
换单元时调用 `syncComment()`（在 advance/back/jump/loadQueue 后）。

- [ ] **Step 2: ExportView 含评语开关**

```ts
// ExportView.vue <script setup>
const includeComments = ref(false);
async function doSaveCsv() {
  errorMsg.value = ""; okMsg.value = "";
  try {
    const path = await save({ title: "保存成绩表 CSV", defaultPath: "成绩表.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await saveCsv(path, includeComments.value);
    okMsg.value = `已保存：${path}`;
  } catch (e) { errorMsg.value = String(e); }
}
```
```html
<!-- 保存按钮旁加开关 -->
<label><input type="checkbox" v-model="includeComments" /> 含每题评语列</label>
```

- [ ] **Step 3: 提交（不跑 npm build）**

```bash
git add src/views/GradeView.vue src/views/ExportView.vue
git commit -m "feat(front): 判分录评语 + 导出可选并入评语列"
```

---

## Task 8: 集成验证与收尾

**Files:** 无新增

- [ ] **Step 1: Rust 全量测试**

Run: `cargo test`
Expected: grading-core 全绿（迁移/add_labeled_page/set_comment/导出评语）。

- [ ] **Step 2: 前端单测**

Run: `npx vitest run`
Expected: 既有 reducer 测全绿（本片未改 reducer，应无回归）。

- [ ] **Step 3: 提醒用户手动构建 + 端到端人工验收**

**不要跑 `npm run build`。** 在报告里提醒用户手动执行 `npm run build`，并人工验收：
1. 建/开一场考试，Setup 建好题目(定 N)。
2. "导入 PDF 文件夹" → 选一个每份 PDF 以人名命名的目录 → 每份 PDF 转成图、自动建学生+已标注页；花名册出现这些人；可改名/删。
3. "标注"确认总表：页数=N+1(姓名页+N题)的绿、对不上的红。
4. "判分"选题 → 左侧显示对应真图 → 批分 + 在评语框写评语。
5. "导出" → 勾"含每题评语列" → 存 CSV → 打开确认：分列在前、评语列在后、内容正确。

- [ ] **Step 4: tag**

```bash
git tag pdf-import-comments
```

---

## Self-Review（对照决策的覆盖）

- **PDF 转图前端 pdf.js**：Task 5 usePdf + Task 6 流程 ✓
- **每 PDF=一学生(文件名)、page_index=problem_number(0=姓名页)**：Task 6 循环 + Task 4 save_pdf_page + Task 2 add_labeled_page ✓
- **复用全流程(判分/导出/确认总表/缺考不改)**：建 labeled page 后天然复用 ✓
- **学生可编辑**：Task 4 rename/delete + Task 6 UI ✓
- **score.comment 幂等迁移兼容既有库**：Task 1 ✓
- **每题评语录入**：Task 7 GradeView（+GradingUnit.comment 补充已在 Task 7 标注）✓
- **CSV 含评语开关、分前评语后**：Task 3 export_to_csv + Task 7 ExportView 开关 ✓
- **构建卡死规避**：全前端任务不跑 npm build，用户手动 ✓

**跨任务补丁（务必执行）**：
1. `save_csv` 命令签名加 `include_comments: bool` 并传给 `export_to_csv`——在 Task 4 改（Rust），Task 5 前端封装配套。
2. `GradingUnit` 加 `comment` 字段 + `build_queue` 查出来 + types.ts 同步——在 Task 3（Rust）或 Task 7 一并做，供判分显示/回填评语。

**已知后续缺口（非本片）**：pdf 渲染大图经 IPC 传 number[] 较重（可后续改二进制通道/降 scale）；OCR 自动识别；题内步级诊断。
