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
            , commands::export_summary, commands::save_csv
            , commands::ingest_folder, commands::read_image, commands::list_pages,
              commands::set_page_label, commands::add_student, commands::labeling_summary
            , commands::list_pdfs, commands::read_pdf, commands::save_pdf_page,
              commands::set_comment, commands::rename_student, commands::delete_student
            , commands::add_problem, commands::add_preset, commands::delete_problem, commands::set_problem_max
            , commands::shift_student_problems, commands::delete_preset, commands::set_problem_rubric
            , commands::save_export_file, commands::import_roster_csv, commands::exam_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
