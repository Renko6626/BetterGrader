import { describe, it, expect } from "vitest";
import { initialGradeState, reduceGradeKey } from "./useGradeKeys";
import type { GradeCtx } from "./useGradeKeys";

const ctx: GradeCtx = { queueLength: 5, peekMin: -2, peekMax: 2 };
const s0 = initialGradeState();

describe("爽批态", () => {
  it("数字键落档位分但不前进（Enter 才前进）", () => {
    const r = reduceGradeKey(s0, "3", ctx);
    expect(r.effect).toEqual({ kind: "setPreset", slot: 3 });
    expect(r.state.index).toBe(0); // 不动
  });
  it("Enter 提交并前进到下一学生", () => {
    const r = reduceGradeKey(s0, "Enter", ctx);
    expect(r.effect).toEqual({ kind: "advance" });
    expect(r.state.index).toBe(1);
  });
  it("Backspace 回上一份（index 夹在 0）", () => {
    const r = reduceGradeKey({ ...s0, index: 2 }, "Backspace", ctx);
    expect(r.effect).toEqual({ kind: "back" });
    expect(r.state.index).toBe(1);
    expect(reduceGradeKey(s0, "Backspace", ctx).state.index).toBe(0);
  });
  it("←/→ 只读速览，夹在 peek 边界；↓/Esc 复位", () => {
    const r1 = reduceGradeKey(s0, "ArrowRight", ctx);
    expect(r1.state.peek).toBe(1);
    expect(r1.effect).toEqual({ kind: "none" }); // 只读，不改分不换单元
    const rMax = reduceGradeKey({ ...s0, peek: 2 }, "ArrowRight", ctx);
    expect(rMax.state.peek).toBe(2); // 夹住
    expect(reduceGradeKey({ ...s0, peek: 2 }, "ArrowDown", ctx).state.peek).toBe(0);
    expect(reduceGradeKey({ ...s0, peek: -1 }, "Escape", ctx).state.peek).toBe(0);
  });
  it("F 存疑、J 跳存疑、G 开队列总览", () => {
    expect(reduceGradeKey(s0, "f", ctx).effect).toEqual({ kind: "flag" });
    expect(reduceGradeKey(s0, "j", ctx).effect).toEqual({ kind: "nextFlag" });
    expect(reduceGradeKey(s0, "g", ctx).state.overview).toBe(true);
  });
  it("换单元时 peek 与 manual 复位", () => {
    const r = reduceGradeKey({ ...s0, peek: 2, manual: true, buffer: "1" }, "Enter", ctx);
    expect(r.state.peek).toBe(0);
    expect(r.state.manual).toBe(false);
    expect(r.state.buffer).toBe("");
  });
  it("advance 换单元时 peek 复位为 0", () => {
    const r = reduceGradeKey({ ...s0, peek: 2 }, "Enter", ctx);
    expect(r.effect).toEqual({ kind: "advance" });
    expect(r.state.index).toBe(1);
    expect(r.state.peek).toBe(0);      // moveTo 复位
  });
  it("back 换单元时 peek/manual/buffer 复位", () => {
    const r = reduceGradeKey({ ...s0, index: 2, peek: -1 }, "Backspace", ctx);
    expect(r.effect).toEqual({ kind: "back" });
    expect(r.state.index).toBe(1);
    expect(r.state.peek).toBe(0);
  });
  it("空格 = 提交并前进（同 Enter 轴1）", () => {
    const r = reduceGradeKey(s0, " ", ctx);
    expect(r.effect).toEqual({ kind: "advance" });
    expect(r.state.index).toBe(1);
  });
  it("← 速览递减并夹在 peekMin", () => {
    expect(reduceGradeKey(s0, "ArrowLeft", ctx).state.peek).toBe(-1);
    const atMin = reduceGradeKey({ ...s0, peek: ctx.peekMin }, "ArrowLeft", ctx);
    expect(atMin.state.peek).toBe(ctx.peekMin);       // 夹住
    expect(atMin.effect).toEqual({ kind: "none" });   // 只读
  });
  it("Enter 在队列末尾不越界", () => {
    const last = ctx.queueLength - 1;
    const r = reduceGradeKey({ ...s0, index: last }, "Enter", ctx);
    expect(r.state.index).toBe(last);   // 夹在末尾
  });
});

describe("手动模式（M/0 进入）与上下文 Enter（修复的 bug）", () => {
  it("M 进入手动、数字进 buffer、不落分不前进", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    expect(s.manual).toBe(true);
    s = reduceGradeKey(s, "1", ctx).state;
    s = reduceGradeKey(s, "3", ctx).state;
    expect(s.buffer).toBe("13");
    expect(s.index).toBe(0); // 没前进
  });
  it("0 也能进入手动；进手动后 0 当数字", () => {
    let s = reduceGradeKey(s0, "0", ctx).state;
    expect(s.manual).toBe(true);
    s = reduceGradeKey(s, "0", ctx).state;
    expect(s.buffer).toBe("0");
  });
  it("手动 Enter = 确认数字并留在原地（不前进）", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    s = reduceGradeKey(s, "1", ctx).state;
    s = reduceGradeKey(s, "3", ctx).state;
    const r = reduceGradeKey(s, "Enter", ctx);
    expect(r.effect).toEqual({ kind: "setManual", value: 13 });
    expect(r.state.index).toBe(0);      // 留在当前单元
    expect(r.state.manual).toBe(false); // 退出手动
    expect(r.state.buffer).toBe("");
  });
  it("手动 Backspace 编辑 buffer，不回上一份", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    s = reduceGradeKey(s, "1", ctx).state;
    s = reduceGradeKey(s, "3", ctx).state;
    const r = reduceGradeKey(s, "Backspace", ctx);
    expect(r.effect).toEqual({ kind: "none" });
    expect(r.state.buffer).toBe("1");
  });
  it("手动 Esc 取消，不落分", () => {
    let s = reduceGradeKey(s0, "m", ctx).state;
    s = reduceGradeKey(s, "5", ctx).state;
    const r = reduceGradeKey(s, "Escape", ctx);
    expect(r.effect).toEqual({ kind: "none" });
    expect(r.state.manual).toBe(false);
    expect(r.state.buffer).toBe("");
  });
});

describe("队列总览开启时抑制判分键", () => {
  it("总览开时数字键不落分，Esc 关闭", () => {
    const open = { ...s0, overview: true };
    expect(reduceGradeKey(open, "3", ctx).effect).toEqual({ kind: "none" });
    expect(reduceGradeKey(open, "Escape", ctx).state.overview).toBe(false);
  });
});
