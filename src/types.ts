export interface Problem { id: number; number: number; title: string; max_score: number }
export interface Preset { id: number; slot: number; label: string; points: number }
export interface Student { id: number; name: string; exam_number: string | null; roster_order: number | null }
export type ScoreState = "Ungraded" | "Flagged" | "Graded";
export interface GradingUnit {
  student_id: number; student_name: string; problem_id: number; problem_number: number;
  pages: string[]; total: number | null; state: ScoreState; preset_id: number | null;
}
export interface PageRef { problem_number: number; image_path: string }
