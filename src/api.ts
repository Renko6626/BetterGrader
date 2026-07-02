import { invoke } from "@tauri-apps/api/core";
import type { Problem, Preset, Student, GradingUnit, PageRef, ScoreState, ExamInfo, ExportData, PageRow, LabelSummary } from "./types";

// 考试生命周期
export const newExam = (dir: string) => invoke<number>("new_exam", { dir });
export const openExam = (dir: string) => invoke<number>("open_exam", { dir });
export const seedDemoExam = (dir: string) => invoke<number>("seed_demo_exam", { dir });
export const currentExam = () => invoke<ExamInfo | null>("current_exam");

// 作用于当前考试（不再传 examId）
export const listProblems = () => invoke<Problem[]>("list_problems");
export const listPresets = (problemId: number) => invoke<Preset[]>("list_presets", { problemId });
export const listStudents = () => invoke<Student[]>("list_students");
export const addProblem = (number: number, title: string, maxScore: number) =>
  invoke<number>("add_problem", { number, title, maxScore });
export const addPreset = (problemId: number, slot: number, label: string, points: number) =>
  invoke<number>("add_preset", { problemId, slot, label, points });
export const deleteProblem = (problemId: number) => invoke<void>("delete_problem", { problemId });
export const setProblemMax = (problemId: number, maxScore: number) =>
  invoke<void>("set_problem_max", { problemId, maxScore });
export const setProblemRubric = (problemId: number, rubric: string) =>
  invoke<void>("set_problem_rubric", { problemId, rubric });
export const shiftStudentProblems = (studentId: number, delta: number) =>
  invoke<void>("shift_student_problems", { studentId, delta });
export const deletePreset = (presetId: number) => invoke<void>("delete_preset", { presetId });
export const buildQueue = (problemNumber: number) => invoke<GradingUnit[]>("build_queue", { problemNumber });
export const setScore = (studentId: number, problemId: number, total: number | null,
                         presetId: number | null, stateStr: ScoreState) =>
  invoke<void>("set_score", { studentId, problemId, total, presetId, stateStr });
export const studentPages = (studentId: number) => invoke<PageRef[]>("student_pages", { studentId });

// 导出
export const exportSummary = () => invoke<ExportData>("export_summary");
export const saveCsv = (path: string, includeComments: boolean) =>
  invoke<void>("save_csv", { path, includeComments });

// 扫描/理片
export const ingestFolder = (srcDir: string) => invoke<number>("ingest_folder", { srcDir });
export const readImage = (filename: string) => invoke<number[]>("read_image", { filename });
export const listPages = () => invoke<PageRow[]>("list_pages");
export const setPageLabel = (pageId: number, studentId: number | null, problemNumber: number | null) =>
  invoke<void>("set_page_label", { pageId, studentId, problemNumber });
export const addStudent = (name: string, examNumber: string | null) =>
  invoke<number>("add_student", { name, examNumber });
export const labelingSummary = () => invoke<LabelSummary>("labeling_summary");

// PDF 导入 / 评语 / 学生编辑
export const listPdfs = (dir: string) => invoke<string[]>("list_pdfs", { dir });
export const readPdf = (dir: string, filename: string) => invoke<number[]>("read_pdf", { dir, filename });
export const savePdfPage = (studentId: number, pageIndex: number, filename: string, bytes: number[]) =>
  invoke<void>("save_pdf_page", { studentId, pageIndex, filename, bytes });
export const setComment = (studentId: number, problemId: number, comment: string) =>
  invoke<void>("set_comment", { studentId, problemId, comment });
export const renameStudent = (studentId: number, name: string) => invoke<void>("rename_student", { studentId, name });
export const deleteStudent = (studentId: number) => invoke<void>("delete_student", { studentId });
