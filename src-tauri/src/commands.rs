use std::sync::Mutex;
use std::path::PathBuf;
use grading_core::{Db, ExamInfo, Problem, Preset, Student, GradingUnit, PageRef, ScoreState,
                   persist, setup, grading, fake};

pub struct OpenExam {
    pub db: Db,
    #[allow(dead_code)] // 考试目录，供后续 images/ 与导出默认路径用
    pub dir: PathBuf,
    pub exam_id: i64,
}
pub struct AppState(pub Mutex<Option<OpenExam>>);

type R<T> = Result<T, String>;
fn e<E: std::fmt::Display>(x: E) -> String { x.to_string() }

fn set_current(state: &tauri::State<AppState>, dir: PathBuf, db: Db, exam_id: i64) -> R<i64> {
    *state.0.lock().map_err(e)? = Some(OpenExam { db, dir, exam_id });
    Ok(exam_id)
}

/// 新建考试：目录必须为空（无已有考试），否则应改用『打开考试』。
#[tauri::command]
pub fn new_exam(state: tauri::State<AppState>, dir: String) -> R<i64> {
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(dir.join("images")).map_err(e)?;
    let db = Db::open(&dir.join("exam.db")).map_err(e)?;
    if persist::exam_count(&db).map_err(e)? > 0 {
        return Err("该目录已存在考试，请用『打开考试』".into());
    }
    let exam_id = persist::ensure_exam(&db, "未命名考试", "").map_err(e)?;
    set_current(&state, dir, db, exam_id)
}

/// 打开考试：目录必须已有 exam.db 且含一场考试。
#[tauri::command]
pub fn open_exam(state: tauri::State<AppState>, dir: String) -> R<i64> {
    let dir = PathBuf::from(dir);
    if !dir.join("exam.db").exists() {
        return Err("该目录没有考试（缺 exam.db）".into());
    }
    let db = Db::open(&dir.join("exam.db")).map_err(e)?;
    if persist::exam_count(&db).map_err(e)? == 0 {
        return Err("该目录没有考试".into());
    }
    std::fs::create_dir_all(dir.join("images")).map_err(e)?;
    let exam_id = persist::ensure_exam(&db, "未命名考试", "").map_err(e)?;
    set_current(&state, dir, db, exam_id)
}

/// 演示考试：空目录则注入假数据；已有考试则直接打开（不再 seed，守单库单场）。
#[tauri::command]
pub fn seed_demo_exam(state: tauri::State<AppState>, dir: String) -> R<i64> {
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(dir.join("images")).map_err(e)?;
    let db = Db::open(&dir.join("exam.db")).map_err(e)?;
    let exam_id = if persist::exam_count(&db).map_err(e)? == 0 {
        fake::seed_fake_exam(&db).map_err(e)?
    } else {
        persist::ensure_exam(&db, "未命名考试", "").map_err(e)?
    };
    set_current(&state, dir, db, exam_id)
}

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
