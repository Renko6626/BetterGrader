// 把后端/IPC 抛出的错误转成用户能看懂、可操作的中文。
// 未知错误兜底返回原串（仍好过白屏），已知错误给明确下一步。
export function humanizeError(e: unknown): string {
  const raw = typeof e === "string" ? e : (e as { message?: string })?.message ?? String(e);
  if (/no exam open/i.test(raw)) {
    return "尚未打开考试——请先到「考试设置」新建或打开一个考试。";
  }
  if (/该目录已存在考试/.test(raw)) {
    return "这个目录里已经有一场考试了。请换一个空目录新建，或用「打开考试」打开它。";
  }
  if (/非法文件名/.test(raw)) {
    return "文件名不合法（含 / \\ .. : 等字符），已跳过。";
  }
  if (/判分键仅限/.test(raw)) {
    return "判分键只能是 1–9。";
  }
  return raw;
}

// 该错误是否为“未打开考试”——某些视图据此显示引导而非红色报错。
export function isNoExam(e: unknown): boolean {
  const raw = typeof e === "string" ? e : (e as { message?: string })?.message ?? String(e);
  return /no exam open/i.test(raw);
}
