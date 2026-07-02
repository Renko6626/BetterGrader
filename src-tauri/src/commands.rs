use std::sync::Mutex;
use std::path::PathBuf;
use grading_core::{Db, ExamInfo, Problem, Preset, Student, GradingUnit, PageRef, ScoreState, ExportData,
                   persist, setup, grading, fake, export, ingest, label, PageRow, LabelSummary};

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
#[tauri::command] pub fn add_problem(state: tauri::State<AppState>, number: i64, title: String, max_score: i64) -> R<i64> { with_exam(&state, |oe| setup::add_problem(&oe.db, oe.exam_id, number, &title, max_score)) }
#[tauri::command] pub fn add_preset(state: tauri::State<AppState>, problem_id: i64, slot: i64, label: String, points: i64) -> R<i64> { with_exam(&state, |oe| setup::add_preset(&oe.db, problem_id, slot, &label, points)) }
#[tauri::command]
pub fn delete_problem(state: tauri::State<AppState>, problem_id: i64) -> R<()> {
    with_exam(&state, |oe| {
        oe.db.conn.execute("DELETE FROM score_preset WHERE problem_id=?1", [problem_id])?;
        oe.db.conn.execute("DELETE FROM score WHERE problem_id=?1", [problem_id])?;
        oe.db.conn.execute("DELETE FROM problem WHERE id=?1", [problem_id])?;
        Ok(())
    })
}
#[tauri::command]
pub fn set_problem_max(state: tauri::State<AppState>, problem_id: i64, max_score: i64) -> R<()> {
    with_exam(&state, |oe| {
        oe.db.conn.execute("UPDATE problem SET max_score=?2 WHERE id=?1", (problem_id, max_score))?;
        // "满分"档位跟随满分变化，保证一键给满分仍等于满分
        oe.db.conn.execute("UPDATE score_preset SET points=?2 WHERE problem_id=?1 AND label='满分'", (problem_id, max_score))?;
        Ok(())
    })
}
#[tauri::command] pub fn build_queue(state: tauri::State<AppState>, problem_number: i64) -> R<Vec<GradingUnit>> { with_exam(&state, |oe| grading::build_queue(&oe.db, oe.exam_id, problem_number)) }
#[tauri::command] pub fn student_pages(state: tauri::State<AppState>, student_id: i64) -> R<Vec<PageRef>> { with_exam(&state, |oe| grading::student_pages(&oe.db, student_id)) }

#[tauri::command]
pub fn set_score(state: tauri::State<AppState>, student_id: i64, problem_id: i64,
                 total: Option<i64>, preset_id: Option<i64>, state_str: String) -> R<()> {
    with_exam(&state, |oe| grading::set_score(&oe.db, student_id, problem_id, total, preset_id, ScoreState::from_str(&state_str)))
}

#[tauri::command]
pub fn export_summary(state: tauri::State<AppState>) -> R<ExportData> {
    with_exam(&state, |oe| export::build_export(&oe.db, oe.exam_id))
}

#[tauri::command]
pub fn save_csv(state: tauri::State<AppState>, path: String, include_comments: bool) -> R<()> {
    let csv = with_exam(&state, |oe| Ok(export::export_to_csv(&export::build_export(&oe.db, oe.exam_id)?, include_comments)))?;
    // 加 UTF-8 BOM，Excel 直接识别中文
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(csv.as_bytes());
    std::fs::write(&path, bytes).map_err(e)
}

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
        // 单张失败则跳过、继续（不因一张坏图中止整批）；seq 仍推进以保序/唯一
        if std::fs::copy(src.join(&name), images.join(&dest_name)).is_err() { seq += 1; continue; }
        if ingest::add_ingested_page(&oe.db, oe.exam_id, &dest_name, seq).is_err() { seq += 1; continue; }
        seq += 1; count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub fn read_image(state: tauri::State<AppState>, filename: String) -> R<Vec<u8>> {
    let guard = state.0.lock().map_err(e)?;
    let oe = guard.as_ref().ok_or_else(|| "no exam open".to_string())?;
    // 路径限定 images/ 内：拒绝分隔符/上跳
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") || filename.contains(':') {
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
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") || filename.contains(':') { return Err("非法文件名".into()); }
    std::fs::read(std::path::Path::new(&dir).join(&filename)).map_err(e)
}
#[tauri::command]
pub fn save_pdf_page(state: tauri::State<AppState>, student_id: i64, page_index: i64, filename: String, bytes: Vec<u8>) -> R<()> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") || filename.contains(':') { return Err("非法文件名".into()); }
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
