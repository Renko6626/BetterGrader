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
