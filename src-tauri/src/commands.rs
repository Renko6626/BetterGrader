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
