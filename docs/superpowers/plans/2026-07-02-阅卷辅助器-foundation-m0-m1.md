# 阅卷辅助器 · 地基 + M0 + M1 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个可运行的桌面 App——能建一场假考试（题目、满分、快捷档位、花名册），并对它全键盘横向批完一道题，验证判分循环（命门）的爽感。

**Architecture:** Rust 纯逻辑 crate `grading-core`（域模型 + SQLite，`cargo test` 全覆盖）经 Tauri IPC 暴露给 Vue3 前端。判分键盘逻辑抽成**纯 TypeScript reducer**（`useGradeKeys`），与 Vue 渲染解耦，用 Vitest 单测——因为它是命门、必须可测。图像面板缺图时降级为带标签占位块，使 M1 无需真实扫描图即可验收。

**Tech Stack:** Tauri v2 · Rust + `rusqlite`(bundled) · Vue3 + TypeScript + Vite · Vitest + @vue/test-utils · Pinia(轻量状态)。

## Global Constraints

以下为全项目约束，每个任务的要求都隐含包含本节（数值/措辞照抄自 spec）：

- **判分单元 = `(学生, 题号)`，`score` 只存一个总分数字**（`total`）。不录题内步级。
- **`rubric_step` / `score_item` 建表占位、v1 绝不录入**（前向兼容留门）。
- **判分单元三态**：`Ungraded`(未判) / `Flagged`(存疑) / `Graded`(已判)；外加学生层 `缺考`（花名册有、无 page 绑定）。**任何导出/统计绝不把未判/存疑静默当 0。**（M0/M1 只建三态模型，导出在后续计划。）
- **`Enter` 上下文相关**：手动输入框内 = 确认数字并**留在当前单元**；爽批态 = 提交并前进。**绝不一键两义。**
- **档位是每题各自的绝对数字**，`满分`(=max)、`零分`(=0)、`空白`(=0) 自动预置，其余手敲；不做跨题模板、不做相对表达式。
- **`score.preset_id` 必存**（手动输入时为 NULL）——它是白捡的粗粒度步级诊断信号。
- **数据不出本地**：一场考试 = 一个自包含目录 `/某场考试/{images/, exam.db}`；图导入时拷进 `images/`。
- **AI 永远不能写 `score.total`**（M0/M1 无 AI，仅声明为不可违反的架构约束）。
- **视觉**：monolith 设计系统——冷、减法、高信息密度、几何。判分界面靠键盘，不靠花哨组件。
- **§7.0 承重不变量**（本计划不实现 Label，但 schema 与 core 需为其留位）：叠内答题页与题号严格一一对应，缺答塞空白页占位，软件页数≠N 报警强制指认、绝不盲目自增。

---

## File Structure

```
BetterGrader/
├─ Cargo.toml                     # Rust workspace
├─ package.json                   # 前端依赖 + tauri 脚本
├─ vite.config.ts
├─ vitest.config.ts
├─ index.html
├─ tsconfig.json
├─ crates/
│  └─ grading-core/               # 纯逻辑 crate：域模型 + SQLite
│     ├─ Cargo.toml
│     ├─ src/
│     │  ├─ lib.rs                 # 公开 API 汇出
│     │  ├─ db.rs                  # 打开/迁移；schema.sql 内嵌
│     │  ├─ schema.sql             # 全部建表（含占位表）
│     │  ├─ models.rs              # 结构体 + serde + ScoreState 枚举
│     │  ├─ setup.rs               # Exam Setup 域逻辑（题/档位/花名册）
│     │  ├─ grading.rs             # 判分队列 + 打分 + 三态
│     │  └─ fake.rs                # 假数据 seeder（M1 验收用，"厚"数据）
│     └─ (单测内嵌于各 src 文件 #[cfg(test)])
├─ src-tauri/                     # Tauri 外壳
│  ├─ Cargo.toml
│  ├─ build.rs
│  ├─ tauri.conf.json
│  └─ src/
│     ├─ main.rs                  # 注册 commands、管理 Db 状态
│     └─ commands.rs              # IPC：薄封装转调 grading-core
└─ src/                           # Vue 前端
   ├─ main.ts
   ├─ App.vue                     # 路由：Setup / Grade
   ├─ types.ts                    # 与 Rust models 对应的 TS 类型
   ├─ api.ts                      # 类型化 invoke 封装
   ├─ composables/
   │  └─ useGradeKeys.ts          # 纯键盘 reducer（命门，Vitest 重测）
   └─ views/
      ├─ SetupView.vue            # 题目/档位/花名册 UI
      └─ GradeView.vue            # 单屏判分：图像 + 档位面板 + 键盘 + 队列总览
```

**责任边界**：`grading-core` 不知道 Tauri/前端存在（可独立 `cargo test`）；`commands.rs` 只做序列化转调，无业务逻辑；`useGradeKeys.ts` 是无副作用 reducer（输入 state+key→输出 state+effects），`GradeView.vue` 负责把 effects 落到后端与渲染。

---

## Task 1: 脚手架——Tauri v2 + Vue3 + Rust workspace 可跑

**Files:**
- Create: `Cargo.toml`, `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.ts`, `src/App.vue`
- Create: `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`
- Create: `crates/grading-core/Cargo.toml`, `crates/grading-core/src/lib.rs`

**Interfaces:**
- Produces: 一个能 `npm run tauri dev` 打开窗口的工程骨架；workspace 含 `grading-core` 与 `src-tauri` 两个 crate。

- [ ] **Step 1: 建 Rust workspace 根 `Cargo.toml`**

```toml
# Cargo.toml
[workspace]
members = ["crates/grading-core", "src-tauri"]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
anyhow = "1"
```

- [ ] **Step 2: 建 `grading-core` crate 骨架**

```toml
# crates/grading-core/Cargo.toml
[package]
name = "grading-core"
edition.workspace = true
version.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
rusqlite.workspace = true
anyhow.workspace = true
```

```rust
// crates/grading-core/src/lib.rs
pub mod db;
pub mod models;
pub mod setup;
pub mod grading;
pub mod fake;

pub use db::Db;
pub use models::*;
```

创建空模块占位（后续任务填充），先让它编译：

```rust
// crates/grading-core/src/db.rs
pub struct Db;
```
```rust
// crates/grading-core/src/models.rs
```
```rust
// crates/grading-core/src/setup.rs
```
```rust
// crates/grading-core/src/grading.rs
```
```rust
// crates/grading-core/src/fake.rs
```

- [ ] **Step 3: 建 Tauri 外壳 crate**

```toml
# src-tauri/Cargo.toml
[package]
name = "better-grader"
edition.workspace = true
version.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
grading-core = { path = "../crates/grading-core" }
```

```rust
// src-tauri/build.rs
fn main() {
    tauri_build::build()
}
```

```json
// src-tauri/tauri.conf.json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "BetterGrader",
  "version": "0.1.0",
  "identifier": "com.bettergrader.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [{ "title": "阅卷辅助器", "width": 1280, "height": 860 }],
    "security": { "csp": null }
  },
  "bundle": { "active": true, "targets": "all" }
}
```

```rust
// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 建前端骨架**

```json
// package.json
{
  "name": "better-grader-frontend",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vue-tsc --noEmit && vite build",
    "tauri": "tauri",
    "test": "vitest run"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "pinia": "^2",
    "vue": "^3.4"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@vitejs/plugin-vue": "^5",
    "@vue/test-utils": "^2",
    "happy-dom": "^14",
    "typescript": "^5.4",
    "vite": "^5",
    "vitest": "^1.6",
    "vue-tsc": "^2"
  }
}
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
});
```

```ts
// vitest.config.ts
import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  test: { environment: "happy-dom", globals: true },
});
```

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "jsx": "preserve",
    "lib": ["ES2020", "DOM"],
    "types": ["vitest/globals"],
    "skipLibCheck": true
  },
  "include": ["src/**/*.ts", "src/**/*.vue"]
}
```

```html
<!-- index.html -->
<!doctype html>
<html lang="zh">
  <head><meta charset="UTF-8" /><title>阅卷辅助器</title></head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

```ts
// src/main.ts
import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";

createApp(App).use(createPinia()).mount("#app");
```

```vue
<!-- src/App.vue -->
<script setup lang="ts">
import { ref } from "vue";
import SetupView from "./views/SetupView.vue";
import GradeView from "./views/GradeView.vue";
const route = ref<"setup" | "grade">("setup");
</script>

<template>
  <nav>
    <button @click="route = 'setup'">考试设置</button>
    <button @click="route = 'grade'">判分</button>
  </nav>
  <SetupView v-if="route === 'setup'" />
  <GradeView v-else />
</template>
```

创建两个占位视图，先让它编译：

```vue
<!-- src/views/SetupView.vue -->
<template><div>Setup（待实现）</div></template>
```
```vue
<!-- src/views/GradeView.vue -->
<template><div>Grade（待实现）</div></template>
```

- [ ] **Step 5: 装依赖、验证可编译可跑**

Run: `npm install && cargo build`
Expected: 两者均成功（首次 `cargo build` 会编译 bundled SQLite，较慢属正常）。

Run: `npm run tauri dev`
Expected: 弹出标题"阅卷辅助器"的窗口，顶部有"考试设置/判分"两个按钮，可切换两个占位视图。手动确认后 Ctrl-C 关闭。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: 脚手架——Tauri v2 + Vue3 + grading-core workspace 可跑"
```

---

## Task 2: SQLite schema 与 Db 打开/迁移

**Files:**
- Modify: `crates/grading-core/src/db.rs`
- Create: `crates/grading-core/src/schema.sql`

**Interfaces:**
- Produces:
  - `Db::open_in_memory() -> anyhow::Result<Db>`
  - `Db::open(path: &std::path::Path) -> anyhow::Result<Db>`
  - `Db` 内含 `pub conn: rusqlite::Connection`
  - 建好 spec §4.2 全部表：`exam, problem, score_preset, student, page, score, rubric_step, score_item`

- [ ] **Step 1: 写 schema.sql（照抄 spec §4.2，加占位表）**

```sql
-- crates/grading-core/src/schema.sql
CREATE TABLE IF NOT EXISTS exam (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, date TEXT
);
CREATE TABLE IF NOT EXISTS problem (
  id INTEGER PRIMARY KEY, exam_id INTEGER NOT NULL,
  number INTEGER NOT NULL, title TEXT, max_score INTEGER NOT NULL,
  UNIQUE(exam_id, number)
);
-- 每题快捷档位，绑数字键槽 1..9；满分/零分/空白 自动预置
CREATE TABLE IF NOT EXISTS score_preset (
  id INTEGER PRIMARY KEY, problem_id INTEGER NOT NULL,
  slot INTEGER NOT NULL, label TEXT NOT NULL, points INTEGER NOT NULL,
  UNIQUE(problem_id, slot)
);
-- 学生来自花名册；无 page 绑定者 = 缺考
CREATE TABLE IF NOT EXISTS student (
  id INTEGER PRIMARY KEY, exam_id INTEGER NOT NULL,
  name TEXT NOT NULL, exam_number TEXT, roster_order INTEGER
);
-- 一张扫描图一条；problem_number = 0 表示姓名页/封面（不判分）
CREATE TABLE IF NOT EXISTS page (
  id INTEGER PRIMARY KEY, exam_id INTEGER NOT NULL,
  student_id INTEGER, problem_number INTEGER,
  image_path TEXT NOT NULL, seq INTEGER, status TEXT
);
-- 判分结果，单元 = (学生,题号)，只存总分 + 状态；preset_id 手动时为 NULL
CREATE TABLE IF NOT EXISTS score (
  id INTEGER PRIMARY KEY,
  student_id INTEGER NOT NULL, problem_id INTEGER NOT NULL,
  total INTEGER, state TEXT NOT NULL DEFAULT 'Ungraded',
  preset_id INTEGER, grader TEXT, submitted_at TEXT,
  UNIQUE(student_id, problem_id)
);
-- 【占位·v1 不录入】未来题内步级诊断
CREATE TABLE IF NOT EXISTS rubric_step (
  id INTEGER PRIMARY KEY, problem_id INTEGER NOT NULL,
  "order" INTEGER, label TEXT, points INTEGER
);
CREATE TABLE IF NOT EXISTS score_item (
  id INTEGER PRIMARY KEY, score_id INTEGER NOT NULL,
  rubric_step_id INTEGER NOT NULL, earned INTEGER
);
```

- [ ] **Step 2: 写失败测试**

```rust
// 追加到 crates/grading-core/src/db.rs 末尾
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opens_in_memory_and_creates_tables() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN
             ('exam','problem','score_preset','student','page','score','rubric_step','score_item')",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 8);
    }
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core opens_in_memory`
Expected: 编译失败或 panic——`Db` 还没有 `open_in_memory` 与 `conn`。

- [ ] **Step 4: 实现 Db**

```rust
// crates/grading-core/src/db.rs （替换文件顶部的占位 struct）
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Db {
    pub conn: Connection,
}

const SCHEMA: &str = include_str!("schema.sql");

impl Db {
    pub fn open_in_memory() -> Result<Db> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }
    pub fn open(path: &Path) -> Result<Db> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }
    fn init(conn: Connection) -> Result<Db> {
        conn.execute_batch(&format!("PRAGMA foreign_keys=ON;\n{SCHEMA}"))?;
        Ok(Db { conn })
    }
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p grading-core opens_in_memory`
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add crates/grading-core/src/db.rs crates/grading-core/src/schema.sql
git commit -m "feat(core): SQLite schema 与 Db 打开/迁移（含占位表）"
```

---

## Task 3: Exam Setup 域逻辑（题目 / 档位 / 花名册）

**Files:**
- Modify: `crates/grading-core/src/models.rs`, `crates/grading-core/src/setup.rs`

**Interfaces:**
- Consumes: `Db`（Task 2）
- Produces（供 Task 4 IPC 调用）:
  - `create_exam(db, name: &str, date: &str) -> Result<i64>`
  - `add_problem(db, exam_id: i64, number: i64, title: &str, max_score: i64) -> Result<i64>`
    —— 副作用：自动插入三个预置档位 `满分`(slot 7,=max) / `零分`(slot 8,=0) / `空白`(slot 9,=0)
  - `add_preset(db, problem_id: i64, slot: i64, label: &str, points: i64) -> Result<i64>`
  - `list_problems(db, exam_id: i64) -> Result<Vec<Problem>>`
  - `list_presets(db, problem_id: i64) -> Result<Vec<Preset>>`
  - `import_roster(db, exam_id: i64, rows: &[RosterRow]) -> Result<usize>`
  - `list_students(db, exam_id: i64) -> Result<Vec<Student>>`
  - 结构体 `Problem { id, number, title, max_score }`、`Preset { id, slot, label, points }`、`Student { id, name, exam_number, roster_order }`、`RosterRow { name, exam_number }`

- [ ] **Step 1: 定义 models**

```rust
// crates/grading-core/src/models.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Problem { pub id: i64, pub number: i64, pub title: String, pub max_score: i64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset { pub id: i64, pub slot: i64, pub label: String, pub points: i64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Student { pub id: i64, pub name: String, pub exam_number: Option<String>, pub roster_order: Option<i64> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RosterRow { pub name: String, pub exam_number: Option<String> }

/// 判分单元三态。字符串序列化，与 SQLite `score.state` 一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreState { Ungraded, Flagged, Graded }

impl ScoreState {
    pub fn as_str(self) -> &'static str {
        match self { ScoreState::Ungraded => "Ungraded", ScoreState::Flagged => "Flagged", ScoreState::Graded => "Graded" }
    }
    pub fn from_str(s: &str) -> ScoreState {
        match s { "Flagged" => ScoreState::Flagged, "Graded" => ScoreState::Graded, _ => ScoreState::Ungraded }
    }
}
```

- [ ] **Step 2: 写失败测试**

```rust
// 追加到 crates/grading-core/src/setup.rs
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
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core setup`
Expected: 编译失败——`create_exam` 等函数未定义。

- [ ] **Step 4: 实现 setup.rs**

```rust
// crates/grading-core/src/setup.rs （置于文件顶部）
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
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p grading-core setup`
Expected: 三个测试全 PASS。

- [ ] **Step 6: Commit**

```bash
git add crates/grading-core/src/models.rs crates/grading-core/src/setup.rs
git commit -m "feat(core): Exam Setup 域逻辑——题目/自动档位/花名册"
```

---

## Task 4: 判分域逻辑（队列 / 打分 / 三态）

**Files:**
- Modify: `crates/grading-core/src/grading.rs`, `crates/grading-core/src/models.rs`

**Interfaces:**
- Consumes: `Db`、`ScoreState`、setup 函数
- Produces（供 Task 6 IPC、Task 8 前端）:
  - `build_queue(db, exam_id: i64, problem_number: i64) -> Result<Vec<GradingUnit>>`
    —— 该题所有学生的判分单元，按 `roster_order` 排队；含每单元的 page 图路径、当前分与状态
  - `set_score(db, student_id: i64, problem_id: i64, total: Option<i64>, preset_id: Option<i64>, state: ScoreState) -> Result<()>`（按 `(student_id, problem_id)` upsert）
  - `student_pages(db, student_id: i64) -> Result<Vec<PageRef>>`（轴2 只读速览用：该生全部页，按 seq）
  - 结构体 `GradingUnit { student_id, student_name, problem_id, problem_number, pages: Vec<String>, total: Option<i64>, state: ScoreState, preset_id: Option<i64> }`、`PageRef { problem_number: i64, image_path: String }`

- [ ] **Step 1: 定义 models（追加）**

```rust
// 追加到 crates/grading-core/src/models.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GradingUnit {
    pub student_id: i64,
    pub student_name: String,
    pub problem_id: i64,
    pub problem_number: i64,
    pub pages: Vec<String>,          // 本 (学生,题号) 的图路径（溢出多张）
    pub total: Option<i64>,
    pub state: ScoreState,
    pub preset_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageRef { pub problem_number: i64, pub image_path: String }
```

- [ ] **Step 2: 写失败测试**

```rust
// 追加到 crates/grading-core/src/grading.rs
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
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p grading-core grading`
Expected: 编译失败——`build_queue`/`set_score` 未定义。

- [ ] **Step 4: 实现 grading.rs**

```rust
// crates/grading-core/src/grading.rs （置于文件顶部）
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
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p grading-core grading`
Expected: 两个测试 PASS。

- [ ] **Step 6: Commit**

```bash
git add crates/grading-core/src/grading.rs crates/grading-core/src/models.rs
git commit -m "feat(core): 判分队列/打分/三态（total 可空、preset_id 留痕）"
```

---

## Task 5: 假数据 seeder（M1 验收用"厚"数据）

**Files:**
- Modify: `crates/grading-core/src/fake.rs`

**Interfaces:**
- Consumes: setup + grading 函数
- Produces:
  - `seed_fake_exam(db) -> Result<i64>`（返回 exam_id）
  - 造出：1 场考试、3 道题（各带一个自定义档位 + 三个自动档位）、5 个学生、**每个学生每题至少一张 page**，其中**第 1 个学生每题都有页（跨题完整集，供轴2 速览验收）**，第 3 个学生第 2 题**溢出两页**（seq 连续）。图路径用不存在的假路径（前端缺图降级为占位块，见 Task 9）。

- [ ] **Step 1: 写失败测试**

```rust
// 追加到 crates/grading-core/src/fake.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, grading::*, setup::*};

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
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p grading-core fake`
Expected: 编译失败——`seed_fake_exam` 未定义。

- [ ] **Step 3: 实现 fake.rs**

```rust
// crates/grading-core/src/fake.rs （置于文件顶部）
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
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p grading-core fake`
Expected: PASS。全 core 测试跑一遍：`cargo test -p grading-core` 应全绿。

- [ ] **Step 5: Commit**

```bash
git add crates/grading-core/src/fake.rs
git commit -m "feat(core): 假数据 seeder——3题5人含跨题完整集与溢出页"
```

---

## Task 6: Tauri IPC 命令 + 前端类型化 api 封装

**Files:**
- Create: `src-tauri/src/commands.rs`, `src/types.ts`, `src/api.ts`
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `grading-core` 全部公开函数
- Produces:
  - IPC 命令（`#[tauri::command]`）：`seed_fake`、`list_problems`、`list_presets`、`list_students`、`build_queue`、`set_score`、`student_pages`
  - 前端 `api.ts`：与上述一一对应的类型化异步函数
  - 全局共享一个 `Db`（`Mutex` 包裹）——单写者，无并发问题

- [ ] **Step 1: 实现 commands.rs（含共享 Db 状态）**

```rust
// src-tauri/src/commands.rs
use std::sync::Mutex;
use grading_core::{Db, Problem, Preset, Student, GradingUnit, PageRef, ScoreState, setup, grading, fake};

pub struct AppState(pub Mutex<Db>);

type R<T> = Result<T, String>;
fn e<E: std::fmt::Display>(x: E) -> String { x.to_string() }

#[tauri::command]
pub fn seed_fake(state: tauri::State<AppState>) -> R<i64> {
    let db = state.0.lock().map_err(e)?;
    fake::seed_fake_exam(&db).map_err(e)
}

#[tauri::command]
pub fn list_problems(state: tauri::State<AppState>, exam_id: i64) -> R<Vec<Problem>> {
    let db = state.0.lock().map_err(e)?;
    setup::list_problems(&db, exam_id).map_err(e)
}

#[tauri::command]
pub fn list_presets(state: tauri::State<AppState>, problem_id: i64) -> R<Vec<Preset>> {
    let db = state.0.lock().map_err(e)?;
    setup::list_presets(&db, problem_id).map_err(e)
}

#[tauri::command]
pub fn list_students(state: tauri::State<AppState>, exam_id: i64) -> R<Vec<Student>> {
    let db = state.0.lock().map_err(e)?;
    setup::list_students(&db, exam_id).map_err(e)
}

#[tauri::command]
pub fn build_queue(state: tauri::State<AppState>, exam_id: i64, problem_number: i64) -> R<Vec<GradingUnit>> {
    let db = state.0.lock().map_err(e)?;
    grading::build_queue(&db, exam_id, problem_number).map_err(e)
}

#[tauri::command]
pub fn set_score(state: tauri::State<AppState>, student_id: i64, problem_id: i64,
                 total: Option<i64>, preset_id: Option<i64>, state_str: String) -> R<()> {
    let db = state.0.lock().map_err(e)?;
    grading::set_score(&db, student_id, problem_id, total, preset_id, ScoreState::from_str(&state_str)).map_err(e)
}

#[tauri::command]
pub fn student_pages(state: tauri::State<AppState>, student_id: i64) -> R<Vec<PageRef>> {
    let db = state.0.lock().map_err(e)?;
    grading::student_pages(&db, student_id).map_err(e)
}
```

- [ ] **Step 2: 注册命令与状态（改 main.rs）**

```rust
// src-tauri/src/main.rs （替换）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
use std::sync::Mutex;
use commands::AppState;
use grading_core::Db;

fn main() {
    // M1 阶段用内存库跑假数据；后续计划改为按考试目录打开 exam.db
    let db = Db::open_in_memory().expect("open db");
    tauri::Builder::default()
        .manage(AppState(Mutex::new(db)))
        .invoke_handler(tauri::generate_handler![
            commands::seed_fake, commands::list_problems, commands::list_presets,
            commands::list_students, commands::build_queue, commands::set_score,
            commands::student_pages
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: 前端类型与 api 封装**

```ts
// src/types.ts
export interface Problem { id: number; number: number; title: string; max_score: number }
export interface Preset { id: number; slot: number; label: string; points: number }
export interface Student { id: number; name: string; exam_number: string | null; roster_order: number | null }
export type ScoreState = "Ungraded" | "Flagged" | "Graded";
export interface GradingUnit {
  student_id: number; student_name: string; problem_id: number; problem_number: number;
  pages: string[]; total: number | null; state: ScoreState; preset_id: number | null;
}
export interface PageRef { problem_number: number; image_path: string }
```

```ts
// src/api.ts
import { invoke } from "@tauri-apps/api/core";
import type { Problem, Preset, Student, GradingUnit, PageRef, ScoreState } from "./types";

export const seedFake = () => invoke<number>("seed_fake");
export const listProblems = (examId: number) => invoke<Problem[]>("list_problems", { examId });
export const listPresets = (problemId: number) => invoke<Preset[]>("list_presets", { problemId });
export const listStudents = (examId: number) => invoke<Student[]>("list_students", { examId });
export const buildQueue = (examId: number, problemNumber: number) =>
  invoke<GradingUnit[]>("build_queue", { examId, problemNumber });
export const setScore = (studentId: number, problemId: number, total: number | null,
                         presetId: number | null, stateStr: ScoreState) =>
  invoke<void>("set_score", { studentId, problemId, total, presetId, stateStr });
export const studentPages = (studentId: number) => invoke<PageRef[]>("student_pages", { studentId });
```

- [ ] **Step 4: 验证编译**

Run: `cargo build && npx vue-tsc --noEmit`
Expected: 均成功。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs src/types.ts src/api.ts
git commit -m "feat(ipc): 暴露 setup/grading 命令 + 前端类型化 api 封装"
```

---

## Task 7: 判分键盘 reducer `useGradeKeys`（命门，Vitest 重测）

**Files:**
- Create: `src/composables/useGradeKeys.ts`, `src/composables/useGradeKeys.test.ts`

**Interfaces:**
- Produces（供 Task 9 GradeView 使用）:
  - `initialGradeState(): GradeState`
  - `reduceGradeKey(state: GradeState, key: string, ctx: GradeCtx): GradeResult`
  - 类型：
    - `GradeState { index; peek; manual; buffer; overview }`
    - `GradeCtx { queueLength; peekMin; peekMax }`
    - `GradeResult { state: GradeState; effect: GradeEffect }`
    - `GradeEffect`（联合）：`{kind:'none'}` | `{kind:'setPreset';slot}` | `{kind:'setManual';value}` | `{kind:'advance'}` | `{kind:'back'}` | `{kind:'flag'}` | `{kind:'nextFlag'}` | `{kind:'jump';index}`

- [ ] **Step 1: 写失败测试（覆盖 §5 全部键位与已修的两个语义 bug）**

```ts
// src/composables/useGradeKeys.test.ts
import { describe, it, expect } from "vitest";
import { initialGradeState, reduceGradeKey } from "./useGradeKeys";
import type { GradeCtx } from "./useGradeKeys";

const ctx: GradeCtx = { queueLength: 5, peekMin: -2, peekMax: 2 };
const s0 = initialGradeState();

describe("爽批态", () => {
  it("数字键落档位分但不前进（Enter 才前进）", () => {
    const r = reduceGradeKey(s0, "3", ctx);
    expect(r.effect).toEqual({ kind: "setPreset", slot: 3 });
    expect(r.state.index).toBe(0); // 不动
  });
  it("Enter 提交并前进到下一学生", () => {
    const r = reduceGradeKey(s0, "Enter", ctx);
    expect(r.effect).toEqual({ kind: "advance" });
    expect(r.state.index).toBe(1);
  });
  it("Backspace 回上一份（index 夹在 0）", () => {
    const r = reduceGradeKey({ ...s0, index: 2 }, "Backspace", ctx);
    expect(r.effect).toEqual({ kind: "back" });
    expect(r.state.index).toBe(1);
    expect(reduceGradeKey(s0, "Backspace", ctx).state.index).toBe(0);
  });
  it("←/→ 只读速览，夹在 peek 边界；↓/Esc 复位", () => {
    const r1 = reduceGradeKey(s0, "ArrowRight", ctx);
    expect(r1.state.peek).toBe(1);
    expect(r1.effect).toEqual({ kind: "none" }); // 只读，不改分不换单元
    const rMax = reduceGradeKey({ ...s0, peek: 2 }, "ArrowRight", ctx);
    expect(rMax.state.peek).toBe(2); // 夹住
    expect(reduceGradeKey({ ...s0, peek: 2 }, "ArrowDown", ctx).state.peek).toBe(0);
    expect(reduceGradeKey({ ...s0, peek: -1 }, "Escape", ctx).state.peek).toBe(0);
  });
  it("F 存疑、J 跳存疑、G 开队列总览", () => {
    expect(reduceGradeKey(s0, "f", ctx).effect).toEqual({ kind: "flag" });
    expect(reduceGradeKey(s0, "j", ctx).effect).toEqual({ kind: "nextFlag" });
    expect(reduceGradeKey(s0, "g", ctx).state.overview).toBe(true);
  });
  it("换单元时 peek 与 manual 复位", () => {
    const r = reduceGradeKey({ ...s0, peek: 2, manual: true, buffer: "1" }, "Enter", ctx);
    expect(r.state.peek).toBe(0);
    expect(r.state.manual).toBe(false);
    expect(r.state.buffer).toBe("");
  });
});

describe("手动模式（M/0 进入）与上下文 Enter（修复的 bug）", () => {
  it("M 进入手动、数字进 buffer、不落分不前进", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    expect(s.manual).toBe(true);
    s = reduceGradeKey(s, "1", ctx).state;
    s = reduceGradeKey(s, "3", ctx).state;
    expect(s.buffer).toBe("13");
    expect(s.index).toBe(0); // 没前进
  });
  it("0 也能进入手动；进手动后 0 当数字", () => {
    let s = reduceGradeKey(s0, "0", ctx).state;
    expect(s.manual).toBe(true);
    s = reduceGradeKey(s, "0", ctx).state;
    expect(s.buffer).toBe("0");
  });
  it("手动 Enter = 确认数字并留在原地（不前进）", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    s = reduceGradeKey(s, "1", ctx).state;
    s = reduceGradeKey(s, "3", ctx).state;
    const r = reduceGradeKey(s, "Enter", ctx);
    expect(r.effect).toEqual({ kind: "setManual", value: 13 });
    expect(r.state.index).toBe(0);      // 留在当前单元
    expect(r.state.manual).toBe(false); // 退出手动
    expect(r.state.buffer).toBe("");
  });
  it("手动 Backspace 编辑 buffer，不回上一份", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    s = reduceGradeKey(s, "1", ctx).state;
    s = reduceGradeKey(s, "3", ctx).state;
    const r = reduceGradeKey(s, "Backspace", ctx);
    expect(r.effect).toEqual({ kind: "none" });
    expect(r.state.buffer).toBe("1");
  });
  it("手动 Esc 取消，不落分", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    s = reduceGradeKey(s, "5", ctx).state;
    const r = reduceGradeKey(s, "Escape", ctx);
    expect(r.effect).toEqual({ kind: "none" });
    expect(r.state.manual).toBe(false);
    expect(r.state.buffer).toBe("");
  });
});

describe("队列总览开启时抑制判分键", () => {
  it("总览开时数字键不落分，Esc 关闭", () => {
    const open = { ...s0, overview: true };
    expect(reduceGradeKey(open, "3", ctx).effect).toEqual({ kind: "none" });
    expect(reduceGradeKey(open, "Escape", ctx).state.overview).toBe(false);
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

Run: `npx vitest run src/composables/useGradeKeys.test.ts`
Expected: 全失败——模块未实现。

- [ ] **Step 3: 实现 reducer**

```ts
// src/composables/useGradeKeys.ts
export interface GradeState {
  index: number;    // 队列当前单元下标
  peek: number;     // 0=锚定当前单元；±k=只读速览该生相邻页
  manual: boolean;  // 手动输入模式
  buffer: string;   // 手动数字缓冲
  overview: boolean;// 队列总览打开
}
export interface GradeCtx { queueLength: number; peekMin: number; peekMax: number }
export type GradeEffect =
  | { kind: "none" }
  | { kind: "setPreset"; slot: number }
  | { kind: "setManual"; value: number }
  | { kind: "advance" }
  | { kind: "back" }
  | { kind: "flag" }
  | { kind: "nextFlag" }
  | { kind: "jump"; index: number };
export interface GradeResult { state: GradeState; effect: GradeEffect }

export function initialGradeState(): GradeState {
  return { index: 0, peek: 0, manual: false, buffer: "", overview: false };
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const none = (state: GradeState): GradeResult => ({ state, effect: { kind: "none" } });
// 换单元时复位速览与手动态
const moveTo = (state: GradeState, index: number, effect: GradeEffect): GradeResult => ({
  state: { ...state, index, peek: 0, manual: false, buffer: "" }, effect,
});

export function reduceGradeKey(state: GradeState, key: string, ctx: GradeCtx): GradeResult {
  // 1) 队列总览打开时，抑制所有判分键，只认 Esc 关闭（跳转由组件点击/emit 处理）
  if (state.overview) {
    if (key === "Escape") return none({ ...state, overview: false });
    return none(state);
  }
  // 2) 手动输入模式
  if (state.manual) {
    if (/^[0-9]$/.test(key)) return none({ ...state, buffer: state.buffer + key });
    if (key === "Backspace") return none({ ...state, buffer: state.buffer.slice(0, -1) });
    if (key === "Escape") return none({ ...state, manual: false, buffer: "" });
    if (key === "Enter") {
      if (state.buffer === "") return none(state); // 空 buffer 不落分
      const value = parseInt(state.buffer, 10);
      // 上下文 Enter：确认数字并留在原地（不前进）
      return { state: { ...state, manual: false, buffer: "" }, effect: { kind: "setManual", value } };
    }
    return none(state);
  }
  // 3) 爽批态
  if (/^[1-9]$/.test(key)) return { state, effect: { kind: "setPreset", slot: parseInt(key, 10) } };
  if (key === "m" || key === "M" || key === "0") return none({ ...state, manual: true, buffer: "" });
  if (key === "Enter" || key === " ")
    return moveTo(state, clamp(state.index + 1, 0, ctx.queueLength - 1), { kind: "advance" });
  if (key === "Backspace")
    return moveTo(state, clamp(state.index - 1, 0, ctx.queueLength - 1), { kind: "back" });
  if (key === "ArrowLeft") return none({ ...state, peek: clamp(state.peek - 1, ctx.peekMin, ctx.peekMax) });
  if (key === "ArrowRight") return none({ ...state, peek: clamp(state.peek + 1, ctx.peekMin, ctx.peekMax) });
  if (key === "ArrowDown" || key === "Escape") return none({ ...state, peek: 0 });
  if (key === "f" || key === "F") return { state, effect: { kind: "flag" } };
  if (key === "j" || key === "J") return { state, effect: { kind: "nextFlag" } };
  if (key === "g" || key === "G") return none({ ...state, overview: true });
  return none(state);
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `npx vitest run src/composables/useGradeKeys.test.ts`
Expected: 全 PASS。

- [ ] **Step 5: Commit**

```bash
git add src/composables/useGradeKeys.ts src/composables/useGradeKeys.test.ts
git commit -m "feat(grade): 判分键盘纯 reducer + 全键位单测（含 Enter 上下文/存疑/速览）"
```

---

## Task 8: Setup 视图（题目 / 档位 / 花名册）

**Files:**
- Modify: `src/views/SetupView.vue`

**Interfaces:**
- Consumes: `api.ts`（`seedFake`、`listProblems`、`listPresets`、`listStudents`）
- Produces: 一个能一键 seed 假考试、并展示题目+档位+花名册的界面（M0 的可视化验收面。真实录入表单在此基础上后续补，M1 优先靠 seed 跑通判分）。

> 说明：M0 的完整"手工录入"表单较繁，本计划先交付**一键 seed + 只读展示**以解锁 M1 判分验收；结构化录入表单（增删题、编辑档位、导 CSV）在同一视图内后续任务补全。这是刻意的最小切片，非占位。

- [ ] **Step 1: 实现 SetupView（seed + 展示）**

```vue
<!-- src/views/SetupView.vue -->
<script setup lang="ts">
import { ref } from "vue";
import { seedFake, listProblems, listPresets, listStudents } from "../api";
import type { Problem, Preset, Student } from "../types";

const examId = ref<number | null>(null);
const problems = ref<Problem[]>([]);
const presetsByProblem = ref<Record<number, Preset[]>>({});
const students = ref<Student[]>([]);

async function seed() {
  examId.value = await seedFake();
  problems.value = await listProblems(examId.value);
  for (const p of problems.value) presetsByProblem.value[p.id] = await listPresets(p.id);
  students.value = await listStudents(examId.value);
}
</script>

<template>
  <section class="setup">
    <button @click="seed">载入假考试（M1 验收数据）</button>
    <p v-if="examId">exam_id = {{ examId }}（判分视图会用这场）</p>

    <div v-for="p in problems" :key="p.id" class="problem">
      <h3>题{{ p.number }} · {{ p.title }}（满分 {{ p.max_score }}）</h3>
      <ul>
        <li v-for="pr in presetsByProblem[p.id]" :key="pr.id">
          键 {{ pr.slot }} → {{ pr.label }} = {{ pr.points }} 分
        </li>
      </ul>
    </div>

    <h3 v-if="students.length">花名册（{{ students.length }} 人）</h3>
    <ol>
      <li v-for="s in students" :key="s.id">{{ s.name }}（{{ s.exam_number }}）</li>
    </ol>
  </section>
</template>

<style scoped>
.setup { padding: 16px; font-family: ui-monospace, monospace; }
.problem { margin: 8px 0; }
</style>
```

- [ ] **Step 2: 跑起来手动验收**

Run: `npm run tauri dev`
点"载入假考试"，确认：显示 3 道题、每题列出档位（含自定义档位与自动的满分/零分/空白）、花名册 5 人。记下 `exam_id`。
Expected: 数据与 Task 5 seeder 一致。

- [ ] **Step 3: Commit**

```bash
git add src/views/SetupView.vue
git commit -m "feat(setup): 一键载入假考试并展示题目/档位/花名册（M0 验收面）"
```

---

## Task 9: 判分视图 GradeView（单屏 · 图像 · 档位面板 · 键盘 · 队列总览）

**Files:**
- Modify: `src/views/GradeView.vue`

**Interfaces:**
- Consumes: `api.ts`、`useGradeKeys.ts`
- Produces: 命门界面。把 `reduceGradeKey` 的 effect 落到后端：`setPreset`→查当前题该 slot 的档位分并 `setScore(Graded, preset_id)`；`setManual`→`setScore(Graded, total, preset_id=null)`；`advance/back`→切单元；`flag`→`setScore(Flagged)`；`nextFlag`→跳到下一个 `Flagged`；`jump`→切到指定 index。图像缺失（`fake://` 或文件不存在）时降级为带标签占位块。

- [ ] **Step 1: 实现 GradeView**

```vue
<!-- src/views/GradeView.vue -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { buildQueue, listPresets, setScore, studentPages } from "../api";
import type { GradingUnit, Preset, PageRef } from "../types";
import {
  initialGradeState, reduceGradeKey,
  type GradeState, type GradeCtx, type GradeEffect,
} from "../composables/useGradeKeys";

// M1：固定用 seed 出来的 exam_id=1、先批第 1 题（真实选题在后续计划）
const examId = 1;
const problemNumber = ref(1);

const queue = ref<GradingUnit[]>([]);
const presets = ref<Preset[]>([]);
const gs = ref<GradeState>(initialGradeState());
const peekPages = ref<PageRef[]>([]);   // 当前学生全部页，供轴2 速览

const current = computed(() => queue.value[gs.value.index]);
const ctx = computed<GradeCtx>(() => ({
  queueLength: queue.value.length,
  peekMin: -(Math.max(peekPages.value.length - 1, 0)),
  peekMax: Math.max(peekPages.value.length - 1, 0),
}));

// 速览时显示哪张图：peek=0 → 当前 (学生,题号) 的首张；否则在该生全部页里偏移
const shownImage = computed(() => {
  if (!current.value) return null;
  if (gs.value.peek === 0) return current.value.pages[0] ?? null;
  const anchor = peekPages.value.findIndex(p => p.problem_number === problemNumber.value);
  const i = (anchor < 0 ? 0 : anchor) + gs.value.peek;
  return peekPages.value[i]?.image_path ?? null;
});
const isRealImage = (path: string | null) =>
  !!path && !path.startsWith("fake://"); // 真实文件用 convertFileSrc；此处仅判断是否占位

async function loadQueue() {
  queue.value = await buildQueue(examId, problemNumber.value);
  presets.value = current.value ? await listPresets(current.value.problem_id) : [];
  await refreshPeek();
}
async function refreshPeek() {
  peekPages.value = current.value ? await studentPages(current.value.student_id) : [];
}

async function applyEffect(eff: GradeEffect) {
  const u = current.value;
  if (!u) return;
  switch (eff.kind) {
    case "setPreset": {
      const pr = presets.value.find(p => p.slot === eff.slot);
      if (!pr) return; // 该 slot 无档位，忽略
      await setScore(u.student_id, u.problem_id, pr.points, pr.id, "Graded");
      u.total = pr.points; u.state = "Graded"; u.preset_id = pr.id;
      break;
    }
    case "setManual":
      await setScore(u.student_id, u.problem_id, eff.value, null, "Graded");
      u.total = eff.value; u.state = "Graded"; u.preset_id = null;
      break;
    case "flag":
      await setScore(u.student_id, u.problem_id, u.total, u.preset_id, "Flagged");
      u.state = "Flagged";
      break;
    case "advance": case "back":
      presets.value = await listPresets(current.value!.problem_id);
      await refreshPeek();
      break;
    case "nextFlag": {
      const next = queue.value.findIndex((x, i) => i > gs.value.index && x.state === "Flagged");
      if (next >= 0) { gs.value = { ...gs.value, index: next, peek: 0, manual: false, buffer: "" }; await refreshPeek(); }
      break;
    }
    case "jump":
      gs.value = { ...gs.value, index: eff.index, peek: 0, manual: false, buffer: "", overview: false };
      await refreshPeek();
      break;
    case "none": break;
  }
}

function onKey(e: KeyboardEvent) {
  const before = gs.value.index;
  const r = reduceGradeKey(gs.value, e.key, ctx.value);
  gs.value = r.state;
  e.preventDefault();
  applyEffect(r.effect);
  if (r.state.index !== before) { /* index 变化由 effect 分支刷新 */ }
}

function jumpFromOverview(i: number) {
  applyEffect({ kind: "jump", index: i });
}

onMounted(() => { window.addEventListener("keydown", onKey); loadQueue(); });
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="grade" v-if="current">
    <div class="pane">
      <div class="img">
        <img v-if="isRealImage(shownImage)" :src="shownImage!" alt="答卷" />
        <div v-else class="placeholder">
          <div>占位图（无真实扫描）</div>
          <div>{{ current.student_name }} · 题{{ problemNumber }}
            <span v-if="gs.peek !== 0">· 速览偏移 {{ gs.peek }}</span>
          </div>
        </div>
      </div>
      <aside class="panel">
        <h3>题{{ current.problem_number }}</h3>
        <ul>
          <li v-for="p in presets" :key="p.id">
            <b>{{ p.slot }}</b> {{ p.label }} {{ p.points }}
          </li>
        </ul>
        <p class="total">当前：{{ current.total ?? "—" }}｜{{ current.state }}</p>
        <p v-if="gs.manual" class="manual">手动输入：{{ gs.buffer || "_" }}（Enter 确认）</p>
      </aside>
    </div>
    <footer>
      本题进度 {{ gs.index + 1 }} / {{ queue.length }}
      ｜[F]存疑 [J]下一存疑 [G]总览 [M/0]手动 [←/→]速览
    </footer>

    <!-- 队列总览（G 打开） -->
    <div v-if="gs.overview" class="overview">
      <h3>队列总览（键 Esc 关闭，点击跳转）</h3>
      <ol>
        <li v-for="(u, i) in queue" :key="u.student_id"
            :class="{ cur: i === gs.index }" @click="jumpFromOverview(i)">
          {{ i + 1 }}. {{ u.student_name }} — {{ u.state }}（{{ u.total ?? "—" }}）
        </li>
      </ol>
    </div>
  </section>
  <section v-else class="grade"><p>队列为空。先到"考试设置"点"载入假考试"。</p></section>
</template>

<style scoped>
.grade { height: 100vh; display: flex; flex-direction: column; font-family: ui-monospace, monospace; color: #d0d0d0; background: #14161a; }
.pane { flex: 1; display: flex; }
.img { flex: 1; display: flex; align-items: center; justify-content: center; }
.img img { max-width: 100%; max-height: 100%; }
.placeholder { border: 1px dashed #555; padding: 40px; text-align: center; color: #888; }
.panel { width: 260px; border-left: 1px solid #333; padding: 12px; }
.panel li { list-style: none; margin: 2px 0; }
.total { margin-top: 12px; font-size: 18px; }
.manual { color: #7fd; }
footer { border-top: 1px solid #333; padding: 8px 12px; font-size: 13px; }
.overview { position: fixed; inset: 10% 25%; background: #1c1f24; border: 1px solid #444; padding: 16px; overflow: auto; }
.overview li { cursor: pointer; padding: 2px 0; }
.overview li.cur { color: #7fd; }
</style>
```

- [ ] **Step 2: 手动验收命门（全键盘走一遍）**

Run: `npm run tauri dev`（先在"考试设置"点"载入假考试"，再切"判分"）
逐项确认：
- 数字键 `1` 落"只列式=4"分（面板"当前"变 4、状态 Graded），**不前进**；`Enter` 才前进到下一学生。
- `M`（或 `0`）进手动，敲 `7` `Enter` → 落 7 分且**留在原地**；再 `Enter` 才前进（验证 Enter 上下文语义）。
- `Backspace` 回上一份；`←/→` 切占位图上的"速览偏移"（翻看该生别的题、纯只读，分不变）；`↓/Esc` 复位。
- `F` 存疑（状态变 Flagged）；`J` 跳到下一个存疑。
- `G` 开队列总览、点某行跳过去、`Esc` 关闭。
Expected: 全部符合；占位块正确显示学生名/题号/速览偏移。

- [ ] **Step 3: Commit**

```bash
git add src/views/GradeView.vue
git commit -m "feat(grade): 单屏判分视图——键盘循环/速览/存疑/队列总览，缺图降级占位"
```

---

## Task 10: 集成验证与里程碑收尾

**Files:** 无新增（全量测试 + 文档勾选）

- [ ] **Step 1: 全量测试**

Run: `cargo test && npx vitest run`
Expected: Rust core 全绿、前端 reducer 单测全绿。

- [ ] **Step 2: M0/M1 验收清单（对照 spec §11）**

手动确认（`npm run tauri dev`）：
- **M0**：设置页能载入一场含题目/满分/档位/花名册的假考试。
- **M1**：全键盘横向批完第 1 题——档位一键落分、`M` 手动 + `Enter` 上下文语义正确、实时总分、`Backspace` 回改、`F` 存疑、`G` 随机跳转、`←/→` 轴2 跨题速览（靠第 1 个学生的跨题页集验证）。

- [ ] **Step 3: 打 tag、收尾 commit**

```bash
git add -A
git commit -m "chore: 地基+M0+M1 完成——判分循环命门可全键盘验收"
git tag m1-grading-loop
```

---

## Self-Review（对照 spec 的覆盖检查）

- **判分单元只存总分 + preset_id**：Task 4 `set_score` / `score` 表 ✓
- **三态 Ungraded/Flagged/Graded、total 可空绝不默认 0**：Task 3 models、Task 4 测试断言 `total==None` ✓
- **占位表 rubric_step/score_item 建表不录**：Task 2 schema ✓
- **Enter 上下文相关（修复的 bug）**：Task 7 测试 `手动 Enter 留原地` + `爽批 Enter 前进` ✓
- **档位自动预置 满分/零分/空白**：Task 3 `add_problem` + 测试 ✓
- **preset_id 手动时 NULL**：Task 4 测试 + Task 9 `setManual` 分支 ✓
- **轴2 只读速览、锚定不误给分**：Task 7 `←/→ effect=none`、Task 9 `shownImage` 只改显示不改分 ✓
- **G 队列随机跳转**：Task 7 overview 抑制键 + Task 9 总览点击 jump ✓
- **M1 假数据够"厚"（跨题完整集 + 溢出）**：Task 5 seeder + 测试 ✓
- **数据不出本地、每场一目录**：Task 6 现用内存库（M1 验收足够）；**按考试目录打开 `exam.db` 属后续 Ingest/Label 计划**，此处显式记为已知缺口，非遗漏。
- **§7.0 承重不变量 / Label / 导出 / Ingest**：**均属后续计划**（本计划范围 = 地基+M0+M1），schema 已为其留位（page.problem_number=0 等）。

**已知的、有意的后续缺口（非占位）**：SetupView 的结构化手工录入表单、按考试目录持久化、Label（M2）、Export（M3）、Ingest（M4）。它们进入本计划的后续兄弟计划。
