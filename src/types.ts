export interface Problem { id: number; number: number; title: string; max_score: number }
export interface Preset { id: number; slot: number; label: string; points: number }
export interface Student { id: number; name: string; exam_number: string | null; roster_order: number | null }
export type ScoreState = "Ungraded" | "Flagged" | "Graded";
export interface GradingUnit {
  student_id: number; student_name: string; problem_id: number; problem_number: number;
  pages: string[]; total: number | null; state: ScoreState; preset_id: number | null;
}
export interface PageRef { problem_number: number; image_path: string }

export interface ExamInfo { id: number; name: string; date: string }
export interface Cell { total: number | null; state: "Graded" | "Flagged" | "Ungraded" | "Absent" }
export interface StudentRow {
  student_id: number; name: string; exam_number: string | null;
  absent: boolean; cells: Cell[]; total: number | null; complete: boolean; rank: number | null;
}
export interface ProblemStat { number: number; max_score: number; avg: number | null; rate: number | null; scored_count: number }
export interface Coverage { roster: number; absent: number; units_total: number; graded: number; flagged: number; ungraded: number }
export interface ExportData {
  exam: ExamInfo; problem_numbers: number[]; problem_max: number[];
  rows: StudentRow[]; problem_stats: ProblemStat[]; coverage: Coverage; ranking_available: boolean;
}
