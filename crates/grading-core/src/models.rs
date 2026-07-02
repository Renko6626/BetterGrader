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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExamInfo { pub id: i64, pub name: String, pub date: String }

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
pub struct PageRow {
    pub id: i64, pub seq: i64, pub image_path: String,
    pub student_id: Option<i64>, pub problem_number: Option<i64>, pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportData {
    pub exam: ExamInfo, pub problem_numbers: Vec<i64>, pub problem_max: Vec<i64>,
    pub rows: Vec<StudentRow>, pub problem_stats: Vec<ProblemStat>,
    pub coverage: Coverage, pub ranking_available: bool,
}
