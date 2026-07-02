export interface LabelState { index: number; currentStudent: number | null; nextProblem: number; picker: boolean }
export interface LabelCtx { pageCount: number }
export type LabelEffect =
  | { kind: "none" }
  | { kind: "openPicker" }
  | { kind: "assign"; studentId: number; problemNumber: number };
export interface LabelResult { state: LabelState; effect: LabelEffect }

export function initialLabelState(): LabelState {
  return { index: 0, currentStudent: null, nextProblem: 1, picker: false };
}
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const none = (state: LabelState): LabelResult => ({ state, effect: { kind: "none" } });

export function reduceLabelKey(state: LabelState, key: string, ctx: LabelCtx): LabelResult {
  if (state.picker) { // 选人态：只认 Esc 关闭；选人由视图调 pickStudent
    if (key === "Escape") return none({ ...state, picker: false });
    return none(state);
  }
  if (key === "ArrowLeft")  return none({ ...state, index: clamp(state.index - 1, 0, ctx.pageCount - 1) });
  if (key === "ArrowRight") return none({ ...state, index: clamp(state.index + 1, 0, ctx.pageCount - 1) });
  if (key === "s" || key === "S") return { state: { ...state, picker: true }, effect: { kind: "openPicker" } };
  if (state.currentStudent == null) return none(state); // 未选学生，派题类键无效
  const sid = state.currentStudent;
  if (key === "Enter" || key === " ") {
    const problemNumber = state.nextProblem;
    return { state: { ...state, nextProblem: state.nextProblem + 1, index: clamp(state.index + 1, 0, ctx.pageCount - 1) },
             effect: { kind: "assign", studentId: sid, problemNumber } };
  }
  if (key === "c" || key === "C") {
    const problemNumber = Math.max(1, state.nextProblem - 1);
    return { state: { ...state, index: clamp(state.index + 1, 0, ctx.pageCount - 1) },
             effect: { kind: "assign", studentId: sid, problemNumber } };
  }
  if (key === "n" || key === "N") { // 跳题：counter 进、停在原页
    return none({ ...state, nextProblem: state.nextProblem + 1 });
  }
  return none(state);
}

// 视图在花名册选人确认后调用：当前页记为该生姓名页(题0)，设当前学生并前进
export function pickStudent(state: LabelState, studentId: number, ctx: LabelCtx): LabelResult {
  return {
    state: { ...state, currentStudent: studentId, nextProblem: 1, picker: false,
             index: clamp(state.index + 1, 0, ctx.pageCount - 1) },
    effect: { kind: "assign", studentId, problemNumber: 0 },
  };
}
