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
