# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

BetterGrader (阅卷辅助器) is a single-user, local-first **Tauri v2 desktop app for grading olympiad exam papers**. The value anchor is the keyboard-driven scoring loop; everything else feeds it. Design docs live in `docs/superpowers/specs/` and implementation plans in `docs/superpowers/plans/` — read the specs (§ numbering is referenced throughout the code/commits) before non-trivial changes.

## Commands

- **Rust tests (fast, use constantly):** `cargo test` (whole workspace) or `cargo test -p grading-core` (core only). Single test: `cargo test -p grading-core <test_name>`. Almost all domain logic is tested here.
- **Frontend unit tests:** `npx vitest run` (single run) or `npx vitest run src/composables/useGradeKeys.test.ts` for one file. Only the pure keyboard reducers are unit-tested.
- **Rust build:** `cargo build` (fine — batch compiler, exits cleanly).
- **Run the app:** `npm run tauri dev`.

### ⚠️ `npm run build` / `vite build` HANGS in this environment
The `vite build` step finishes the bundle (`✓ built in …`) but the node process never exits, gets backgrounded, and orphans — piling up stuck processes ("npm 卡死"). **Do NOT run `npm run build` yourself.** For type/bundle verification, ask the user to run it manually. `cargo`, `cargo test`, `vitest`, and the `vite` dev server (inside `tauri dev`) are unaffected — only batch `vite build` hangs. When dispatching subagents, put this in their instructions.

## Architecture

Three layers, deliberately decoupled:

1. **`crates/grading-core/` — pure domain logic, the single source of truth.** Knows nothing about Tauri or the frontend; compiles and tests standalone. Filesystem/IO stays in the command layer, not here. Key modules: `db` (open + `include_str!` schema + idempotent `migrate`), `setup` (exam/problem/preset/roster), `grading` (queue + `set_score` + `set_comment` + three-state), `export` (`build_export` + `export_to_csv`), `persist` (one-exam-per-db handles), `ingest` (natural sort + create unlabeled pages), `label` (labeling writes + `labeling_summary`), `fake` (demo seeder), `models`.

2. **`src-tauri/src/commands.rs` — thin IPC shim.** Each `#[tauri::command]` locks shared state and delegates to grading-core; no business logic. State is `AppState(Mutex<Option<OpenExam>>)` where `OpenExam { db, dir, exam_id }` — i.e. **"the currently open exam," or none**. Most commands go through the `with_exam` helper (errors `"no exam open"` when none). App-defined commands need NO capability grant (allowed by default); **plugin** commands do — `capabilities/default.json` grants `dialog:allow-open` + `dialog:allow-save`.

3. **`src/` — Vue3 + TypeScript + naive-ui frontend.** Views `SetupView/LabelView/GradeView/ExportView` under a top nav in `App.vue`. `api.ts` = typed `invoke` wrappers (Tauri v2 maps camelCase JS keys → snake_case Rust params). The two **pure keyboard reducers** `useGradeKeys.ts` / `useLabelKeys.ts` are the crux: keyboard→state/effect logic is side-effect-free and Vitest-tested; the view executes the emitted effects (persist + re-render). `usePdf.ts` renders PDFs via pdf.js; `useImage.ts` turns a filename into a blob URL via the `read_image` command.

### The pipeline (spec §3)
`Setup` (exam skeleton: problems + max scores + per-problem quick-score presets + roster) → `Ingest`/`PDF-import` (produce `page` rows) → `Label` (bind each page to `(student, problem)`) → `Grade` (keyboard scoring, horizontal by-problem) → `Export` (CSV real file + printable PDF report).

### Data model & invariants that span files
- **One exam = one self-contained directory** `{exam.db, images/}` (portable/archivable). `page.image_path` stores only the **filename**, resolved against `<dir>/images/` at read time.
- **Grading unit = `(student, problem)`; `score` stores only a total** (`score_item`/`rubric_step` tables exist but are unused placeholders). `score.total` is written by **`set_score` only** — no other path (comments, imports, "AI") ever writes it.
- **Three states** `Ungraded`/`Flagged`/`Graded`, plus student-level `Absent` (缺考 = roster student with no pages). Export **never silently fills 0** for未判/存疑; **ranking is emitted only when every included unit is Graded**.
- **`page.student_id`/`page.problem_number` are nullable** (unlabeled). `problem_number = 0` marks a **name/cover page** — excluded from grading and answer-page counts everywhere.
- **Two import paths converge on the same labeled-page shape:** Ingest creates unlabeled pages that Label fills in; PDF-import auto-labels (each PDF = one student from the filename; `page_index → problem_number`, page 0 = name page). Downstream Grade/Export/§7.0-count-check are identical for both.
- **`read_image`/`read_pdf`/`save_pdf_page` filename guards reject `/ \ .. :`** (the `:` closes Windows drive-relative traversal).

## Conventions
- Reducers stay pure (spreads, no input mutation, no IO) — mirror the existing `useGradeKeys` style for any new keyboard logic, and unit-test it.
- naive-ui components + dark theme for forms/tables/dialogs; the dense keyboard screens use native elements + scoped CSS.
- PDF pages are rendered to **JPEG** (not PNG) before crossing IPC — scanned-page PNGs are multi-MB and `number[]` IPC transfer is the bottleneck.
- `[profile.dev]` in the root `Cargo.toml` strips debuginfo from dependencies to speed builds.
