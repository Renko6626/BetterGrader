import { invoke } from "@tauri-apps/api/core";
import type { Problem, Preset, Student, GradingUnit, PageRef, ScoreState, ExamInfo, ExportData } from "./types";

// 考试生命周期
export const newExam = (dir: string) => invoke<number>("new_exam", { dir });
export const openExam = (dir: string) => invoke<number>("open_exam", { dir });
export const seedDemoExam = (dir: string) => invoke<number>("seed_demo_exam", { dir });
export const currentExam = () => invoke<ExamInfo | null>("current_exam");

// 作用于当前考试（不再传 examId）
export const listProblems = () => invoke<Problem[]>("list_problems");
export const listPresets = (problemId: number) => invoke<Preset[]>("list_presets", { problemId });
export const listStudents = () => invoke<Student[]>("list_students");
export const buildQueue = (problemNumber: number) => invoke<GradingUnit[]>("build_queue", { problemNumber });
export const setScore = (studentId: number, problemId: number, total: number | null,
                         presetId: number | null, stateStr: ScoreState) =>
  invoke<void>("set_score", { studentId, problemId, total, presetId, stateStr });
export const studentPages = (studentId: number) => invoke<PageRef[]>("student_pages", { studentId });

// 导出
export const exportSummary = () => invoke<ExportData>("export_summary");
export const saveCsv = (path: string) => invoke<void>("save_csv", { path });
