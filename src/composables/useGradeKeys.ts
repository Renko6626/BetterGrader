export interface GradeState {
  index: number;    // 队列当前单元下标
  peek: number;     // 0=锚定当前单元；±k=只读速览该生相邻页
  manual: boolean;  // 手动输入模式
  buffer: string;   // 手动数字缓冲
  overview: boolean;// 队列总览打开
}
export interface GradeCtx { queueLength: number; peekMin: number; peekMax: number }
export type GradeEffect =
  | { kind: "none" }
  | { kind: "setPreset"; slot: number }
  | { kind: "setManual"; value: number }
  | { kind: "advance" }
  | { kind: "back" }
  | { kind: "flag" }
  | { kind: "nextFlag" }
  | { kind: "jump"; index: number };
export interface GradeResult { state: GradeState; effect: GradeEffect }

export function initialGradeState(): GradeState {
  return { index: 0, peek: 0, manual: false, buffer: "", overview: false };
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const none = (state: GradeState): GradeResult => ({ state, effect: { kind: "none" } });
// 换单元时复位速览与手动态
const moveTo = (state: GradeState, index: number, effect: GradeEffect): GradeResult => ({
  state: { ...state, index, peek: 0, manual: false, buffer: "" }, effect,
});

export function reduceGradeKey(state: GradeState, key: string, ctx: GradeCtx): GradeResult {
  // 1) 队列总览打开时，抑制所有判分键，只认 Esc 关闭（跳转由组件点击/emit 处理）
  if (state.overview) {
    if (key === "Escape") return none({ ...state, overview: false });
    return none(state);
  }
  // 2) 手动输入模式
  if (state.manual) {
    if (/^[0-9]$/.test(key)) return none({ ...state, buffer: state.buffer + key });
    if (key === "Backspace") return none({ ...state, buffer: state.buffer.slice(0, -1) });
    if (key === "Escape") return none({ ...state, manual: false, buffer: "" });
    if (key === "Enter") {
      if (state.buffer === "") return none(state); // 空 buffer 不落分
      const value = parseInt(state.buffer, 10);
      // 上下文 Enter：确认数字并留在原地（不前进），同时复位速览态
      return { state: { ...state, manual: false, buffer: "", peek: 0 }, effect: { kind: "setManual", value } };
    }
    return none(state);
  }
  // 3) 爽批态
  if (/^[1-9]$/.test(key)) return { state, effect: { kind: "setPreset", slot: parseInt(key, 10) } };
  if (key === "m" || key === "M" || key === "0") return none({ ...state, manual: true, buffer: "" });
  // ←/→ 或 Enter/Backspace：上/下一份（换人）
  if (key === "Enter" || key === " " || key === "ArrowRight")
    return moveTo(state, clamp(state.index + 1, 0, ctx.queueLength - 1), { kind: "advance" });
  if (key === "Backspace" || key === "ArrowLeft")
    return moveTo(state, clamp(state.index - 1, 0, ctx.queueLength - 1), { kind: "back" });
  // ↑/↓：只读速览该生上/下一页；Esc 复位回本题页
  if (key === "ArrowUp") return none({ ...state, peek: clamp(state.peek - 1, ctx.peekMin, ctx.peekMax) });
  if (key === "ArrowDown") return none({ ...state, peek: clamp(state.peek + 1, ctx.peekMin, ctx.peekMax) });
  if (key === "Escape") return none({ ...state, peek: 0 });
  if (key === "f" || key === "F") return { state, effect: { kind: "flag" } };
  if (key === "j" || key === "J") return { state, effect: { kind: "nextFlag" } };
  if (key === "g" || key === "G") return none({ ...state, overview: true });
  return none(state);
}
