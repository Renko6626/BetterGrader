import { invoke } from "@tauri-apps/api/core";
import type { Problem, Preset, Student, GradingUnit, PageRef, ScoreState } from "./types";

export const seedFake = () => invoke<number>("seed_fake");
export const listProblems = (examId: number) => invoke<Problem[]>("list_problems", { examId });
export const listPresets = (problemId: number) => invoke<Preset[]>("list_presets", { problemId });
export const listStudents = (examId: number) => invoke<Student[]>("list_students", { examId });
export const buildQueue = (examId: number, problemNumber: number) =>
  invoke<GradingUnit[]>("build_queue", { examId, problemNumber });
export const setScore = (studentId: number, problemId: number, total: number | null,
                         presetId: number | null, stateStr: ScoreState) =>
  invoke<void>("set_score", { studentId, problemId, total, presetId, stateStr });
export const studentPages = (studentId: number) => invoke<PageRef[]>("student_pages", { studentId });
