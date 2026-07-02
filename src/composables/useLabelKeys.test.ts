import { describe, it, expect } from "vitest";
import { initialLabelState, reduceLabelKey, pickStudent } from "./useLabelKeys";
import type { LabelCtx } from "./useLabelKeys";

const ctx: LabelCtx = { pageCount: 6 };
const s0 = initialLabelState();

describe("Label reducer", () => {
  it("←/→ 只翻页，不派标", () => {
    expect(reduceLabelKey(s0, "ArrowRight", ctx).state.index).toBe(1);
    expect(reduceLabelKey(s0, "ArrowRight", ctx).effect).toEqual({ kind: "none" });
    expect(reduceLabelKey({ ...s0, index: 2 }, "ArrowLeft", ctx).state.index).toBe(1);
    // 夹边界
    expect(reduceLabelKey(s0, "ArrowLeft", ctx).state.index).toBe(0);
  });
  it("S 开花名册选人", () => {
    const r = reduceLabelKey(s0, "s", ctx);
    expect(r.effect).toEqual({ kind: "openPicker" });
    expect(r.state.picker).toBe(true);
  });
  it("选人 → 当前页记姓名页(题0)、设当前学生、nextProblem=1、前进", () => {
    const r = pickStudent({ ...s0, picker: true, index: 0 }, 42);
    expect(r.effect).toEqual({ kind: "assign", studentId: 42, problemNumber: 0 });
    expect(r.state.currentStudent).toBe(42);
    expect(r.state.nextProblem).toBe(1);
    expect(r.state.picker).toBe(false);
    expect(r.state.index).toBe(1); // 前进
  });
  it("Enter 顺序派题号并前进；无当前学生时不派", () => {
    expect(reduceLabelKey(s0, "Enter", ctx).effect).toEqual({ kind: "none" }); // 还没选学生
    let s = { ...s0, currentStudent: 42, nextProblem: 1, index: 1 };
    const r1 = reduceLabelKey(s, "Enter", ctx);
    expect(r1.effect).toEqual({ kind: "assign", studentId: 42, problemNumber: 1 });
    expect(r1.state.nextProblem).toBe(2);
    expect(r1.state.index).toBe(2);
  });
  it("C 接上一题(不进 counter)并前进", () => {
    const s = { ...s0, currentStudent: 42, nextProblem: 4, index: 3 };
    const r = reduceLabelKey(s, "c", ctx);
    expect(r.effect).toEqual({ kind: "assign", studentId: 42, problemNumber: 3 }); // nextProblem-1
    expect(r.state.nextProblem).toBe(4); // 不变
    expect(r.state.index).toBe(4);
  });
  it("N 跳题：counter 进、停在原页", () => {
    const s = { ...s0, currentStudent: 42, nextProblem: 2, index: 3 };
    const r = reduceLabelKey(s, "n", ctx);
    expect(r.effect).toEqual({ kind: "none" });
    expect(r.state.nextProblem).toBe(3); // 声明题2无页，counter 跳到3
    expect(r.state.index).toBe(3);        // 不动
  });
});
