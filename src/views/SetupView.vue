<script setup lang="ts">
import { ref, reactive, computed, onMounted, h } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
  newExam, openExam, seedDemoExam, currentExam, listProblems, listPresets, listStudents, ingestFolder,
  listPdfs, readPdf, savePdfPage, addStudent, renameStudent, deleteStudent,
  addProblem, deleteProblem, setProblemMax, addPreset, deletePreset, setProblemRubric,
  importRosterCsv,
} from "../api";
import type { Problem, Preset, Student, ExamInfo } from "../types";
import { NButton, NCard, NDataTable, NAlert, NSpace, NInputNumber, NProgress } from "naive-ui";
import type { DataTableColumns } from "naive-ui";
import { usePdf } from "../composables/usePdf";

const { renderToJpegs } = usePdf();

const exam = ref<ExamInfo | null>(null);
const problems = ref<Problem[]>([]);
const presetsByProblem = ref<Record<number, Preset[]>>({});
const students = ref<Student[]>([]);
const errorMsg = ref("");
const ingestMsg = ref("");
const pdfMsg = ref("");
const importing = ref(false);
// 长任务进度：done<0 表示"进行中但总数未知"（不确定态），否则显示 done/total 百分比
const prog = ref<{ label: string; done: number; total: number } | null>(null);
const progPct = computed(() =>
  prog.value && prog.value.total > 0 ? Math.round(prog.value.done / prog.value.total * 100) : 0);
// 让出一帧给浏览器重绘，否则重活循环里进度条不会动（看着像卡死）
const paintTick = () => new Promise<void>((r) => setTimeout(r, 0));

// 每题的"加档位"草稿（键槽/名称/分值）
const draft = reactive<Record<number, { slot: number; label: string; points: number }>>({});
function freeSlots(pid: number): number[] {
  const used = new Set((presetsByProblem.value[pid] ?? []).map(pr => pr.slot));
  return [1, 2, 3, 4, 5, 6, 7, 8, 9].filter(s => !used.has(s));
}
function ensureDrafts() {
  for (const p of problems.value) {
    const free = freeSlots(p.id);
    if (!draft[p.id]) draft[p.id] = { slot: free[0] ?? 1, label: "", points: 0 };
    else if (!free.includes(draft[p.id].slot)) draft[p.id].slot = free[0] ?? draft[p.id].slot;
  }
}
async function refresh() {
  exam.value = await currentExam();
  if (!exam.value) { problems.value = []; students.value = []; presetsByProblem.value = {}; return; }
  problems.value = await listProblems();
  const np: Record<number, Preset[]> = {};
  for (const p of problems.value) np[p.id] = await listPresets(p.id);
  presetsByProblem.value = np;
  students.value = await listStudents();
  ensureDrafts();
}
async function doAddPreset(pid: number) {
  const d = draft[pid];
  if (!d || !d.label.trim()) { errorMsg.value = "档位名称不能为空"; return; }
  if (!Number.isInteger(d.slot) || d.slot < 1 || d.slot > 9) { errorMsg.value = "判分键仅限 1-9"; return; }
  errorMsg.value = "";
  try {
    await addPreset(pid, d.slot, d.label.trim(), Math.floor(d.points ?? 0));
    d.label = ""; d.points = 0;
    await refresh();
  } catch (e) { errorMsg.value = String(e); }
}
async function doDeletePreset(presetId: number) {
  errorMsg.value = "";
  try { await deletePreset(presetId); await refresh(); }
  catch (e) { errorMsg.value = String(e); }
}

async function pickDir(): Promise<string | null> {
  const d = await open({ directory: true, multiple: false, title: "选择考试目录" });
  return typeof d === "string" ? d : null;
}
async function doNew()  { await withDir(newExam); }
async function doOpen() { await withDir(openExam); }
async function doDemo() { await withDir(seedDemoExam); }
async function withDir(fn: (dir: string) => Promise<number>) {
  errorMsg.value = "";
  try { const dir = await pickDir(); if (!dir) return; await fn(dir); await refresh(); }
  catch (e) { errorMsg.value = String(e); }
}
async function doIngest() {
  errorMsg.value = ""; ingestMsg.value = "";
  const dir = await open({ directory: true, multiple: false, title: "选择图片文件夹" });
  if (typeof dir !== "string") return;
  // 拷贝在 Rust 里跑，靠 ingest://progress 事件回报进度
  prog.value = { label: "导入图片", done: -1, total: 0 }; // 先"进行中"，收到首个事件才有总数
  const unlisten = await listen<[number, number]>("ingest://progress", (e) => {
    prog.value = { label: "导入图片", done: e.payload[0], total: e.payload[1] };
  });
  try {
    const n = await ingestFolder(dir);
    ingestMsg.value = n > 0
      ? `已导入 ${n} 张图（去"标注"页开始标注）`
      : `没有新增：这些图之前已导入过（按内容去重，不会重复）`;
  } catch (e) { errorMsg.value = String(e); }
  finally { unlisten(); prog.value = null; }
}
async function doImportPdfs() {
  pdfMsg.value = ""; errorMsg.value = "";
  try {
    const dir = await open({ directory: true, multiple: false, title: "选择 PDF 文件夹" });
    if (typeof dir !== "string") return;
    importing.value = true;
    const pdfs = await listPdfs(dir);
    let done = 0; const failed: string[] = [];
    prog.value = { label: "导入 PDF", done: 0, total: pdfs.length };
    for (const name of pdfs) {
      try {
        const studentName = name.replace(/\.pdf$/i, "").trim() || name;
        const bytes = new Uint8Array(await readPdf(dir, name));
        const jpegs = await renderToJpegs(bytes);         // 渲染在前：坏 PDF 在这里失败，不会先建学生
        const sid = await addStudent(studentName, null);
        for (let idx = 0; idx < jpegs.length; idx++) {
          // page_index 0 = 姓名页(题0)，idx = 题号；problem_number = idx
          await savePdfPage(sid, idx, `${sid}_${idx}.jpg`, Array.from(jpegs[idx]));
        }
        done++;
      } catch { failed.push(name); }
      prog.value = { label: "导入 PDF", done: done + failed.length, total: pdfs.length };
      pdfMsg.value = `已导入 ${done}/${pdfs.length}…`;
      await paintTick(); // 让进度条真正刷新，别让渲染霸着主线程
    }
    pdfMsg.value = failed.length
      ? `完成：导入 ${done} 份；失败 ${failed.length} 份（${failed.join("、")}）——可重试这些文件`
      : `完成：导入 ${done} 份 PDF（去"判分"直接开批；页数不符的在"标注"确认总表里修）`;
    await refresh();
  } catch (e) { errorMsg.value = String(e); } finally { importing.value = false; prog.value = null; }
}
async function addOneProblem() {
  errorMsg.value = "";
  const next = problems.value.reduce((m, p) => Math.max(m, p.number), 0) + 1;
  try { await addProblem(next, `题${next}`, 10); await refresh(); } // 默认满分 10，行内再改
  catch (e) { errorMsg.value = String(e); }
}
async function onEditMax(problemId: number, val: number | null) {
  if (val == null || val < 0) return;
  errorMsg.value = "";
  try { await setProblemMax(problemId, Math.floor(val)); await refresh(); }
  catch (e) { errorMsg.value = String(e); }
}
async function onEditRubric(p: Problem) {
  errorMsg.value = "";
  try { await setProblemRubric(p.id, p.rubric ?? ""); }
  catch (e) { errorMsg.value = String(e); }
}
async function doDeleteProblem(id: number) {
  if (window.confirm("删除该题及其档位/已打分数？")) {
    errorMsg.value = "";
    try { await deleteProblem(id); await refresh(); }
    catch (e) { errorMsg.value = String(e); }
  }
}
async function doImportRoster() {
  errorMsg.value = ""; ingestMsg.value = "";
  try {
    const path = await open({ multiple: false, title: "选择花名册 CSV（第一列姓名、第二列学号）",
      filters: [{ name: "CSV", extensions: ["csv", "txt"] }] });
    if (typeof path !== "string") return;
    const n = await importRosterCsv(path);
    ingestMsg.value = n > 0
      ? `已从 CSV 导入 ${n} 名学生（追加到花名册）`
      : `没有新增：CSV 里的学生都已在花名册中（按姓名+学号去重）`;
    await refresh();
  } catch (e) { errorMsg.value = String(e); }
}
async function doRename(sid: number, cur: string) {
  const name = window.prompt("改名", cur);
  if (name && name.trim()) {
    errorMsg.value = "";
    try { await renameStudent(sid, name.trim()); await refresh(); }
    catch (e) { errorMsg.value = String(e); }
  }
}
async function doDelete(sid: number) {
  if (window.confirm("删除该学生及其所有页/分？")) {
    errorMsg.value = "";
    try { await deleteStudent(sid); await refresh(); }
    catch (e) { errorMsg.value = String(e); }
  }
}
const studentColumns: DataTableColumns<Student> = [
  { title: "姓名", key: "name" },
  { title: "考号", key: "exam_number", render: (row) => row.exam_number ?? "—" },
  {
    title: "操作", key: "actions",
    render: (row) =>
      h(NSpace, null, () => [
        h(NButton, { size: "tiny", onClick: () => doRename(row.id, row.name) }, () => "改名"),
        h(NButton, { size: "tiny", type: "error", onClick: () => doDelete(row.id) }, () => "删除"),
      ]),
  },
];
onMounted(refresh);
</script>

<template>
  <section style="padding:16px; font-family:ui-monospace,monospace;">
    <n-space>
      <n-button @click="doNew">新建考试…</n-button>
      <n-button @click="doOpen">打开考试…</n-button>
      <n-button type="primary" @click="doDemo">新建演示考试…</n-button>
      <n-button v-if="exam" @click="doIngest">导入图片文件夹…</n-button>
      <n-button v-if="exam" :loading="importing" :disabled="importing" @click="doImportPdfs">导入 PDF 文件夹…</n-button>
      <n-button v-if="exam" @click="doImportRoster">从 CSV 导入花名册…</n-button>
    </n-space>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px" />
    <n-alert v-if="ingestMsg" type="success" :title="ingestMsg" closable @close="ingestMsg=''" style="margin-top:8px" />
    <n-alert v-if="pdfMsg" type="success" :title="pdfMsg" closable @close="pdfMsg=''" style="margin-top:8px" />
    <div v-if="prog" class="prog-box">
      <span class="prog-label">
        {{ prog.label }}…
        <template v-if="prog.done >= 0 && prog.total > 0">{{ prog.done }} / {{ prog.total }}</template>
        <template v-else>准备中</template>
      </span>
      <n-progress type="line"
        :percentage="prog.done < 0 ? 0 : progPct"
        :processing="prog.done < 0"
        :indicator-placement="'inside'" />
    </div>
    <p v-if="exam">当前：{{ exam.name }}</p>
    <p v-else>未打开考试。点上面按钮选一个目录。</p>

    <div v-if="exam" class="problems">
      <h3>题目设置</h3>
      <p class="hint">每题各自填满分；判分键（1–9）可自定义档位。判分前必须先建题——PDF/图片导入<b>不会</b>自动建题。</p>
      <div v-for="p in problems" :key="p.id" class="prob">
        <div class="prow">
          <b class="pn">题{{ p.number }}</b>
          <span>满分
            <n-input-number :value="p.max_score" :min="0" size="small" style="width:100px"
                            @update:value="(v: number | null) => onEditMax(p.id, v)" />
          </span>
          <n-button size="tiny" type="error" @click="doDeleteProblem(p.id)">删除本题</n-button>
        </div>
        <div class="preset-edit">
          <span class="pl">判分键：</span>
          <span v-for="pr in presetsByProblem[p.id]" :key="pr.id" class="chip">
            <b>{{ pr.slot }}</b> {{ pr.label }}={{ pr.points }}
            <a class="x" title="删除该档位" @click="doDeletePreset(pr.id)">×</a>
          </span>
          <span v-if="draft[p.id] && freeSlots(p.id).length" class="addp">
            ＋键
            <select v-model.number="draft[p.id].slot" class="sel">
              <option v-for="s in freeSlots(p.id)" :key="s" :value="s">{{ s }}</option>
            </select>
            <input v-model="draft[p.id].label" placeholder="名称（如 前两问）" class="il" />
            <input type="number" v-model.number="draft[p.id].points" placeholder="分" class="ip" />
            <n-button size="tiny" @click="doAddPreset(p.id)">加</n-button>
          </span>
        </div>
        <div class="rubric-edit">
          <span class="pl">评分标准：</span>
          <textarea v-model="p.rubric" class="rb" @blur="onEditRubric(p)"
            placeholder="本题评分标准/参考答案（Markdown，可选）——判分时按 R 呼出对照"></textarea>
        </div>
      </div>
      <p v-if="!problems.length" class="hint">还没有题目。</p>
      <n-button size="small" @click="addOneProblem" style="margin-top:8px">＋ 添加一题</n-button>
    </div>
    <n-card v-if="students.length" :title="`花名册（${students.length} 人）`">
      <n-data-table :columns="studentColumns" :data="students" :row-key="(row) => row.id" />
    </n-card>
  </section>
</template>

<style scoped>
.prog-box { margin-top: 10px; padding: 10px; border: 1px solid #3a5570; background: #161c24; }
.prog-label { display: block; font-size: 13px; color: #cfe3ff; margin-bottom: 6px; }
.problems { margin: 12px 0; padding: 10px; border: 1px solid #333; }
.problems h3 { margin: 0 0 6px; }
.hint { color: #9aa0a6; font-size: 12px; margin: 4px 0; }
.prob { border-top: 1px solid #2a2d33; padding: 8px 0; }
.prow { display: flex; align-items: center; gap: 14px; }
.prow .pn { min-width: 3em; }
.preset-edit { margin-top: 6px; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: 12px; color: #b8bdc4; }
.preset-edit .pl { color: #9aa0a6; }
.chip { border: 1px solid #3a3f47; border-radius: 3px; padding: 1px 6px; }
.chip b { color: #7fd; margin-right: 4px; }
.chip .x { color: #f77; cursor: pointer; margin-left: 6px; text-decoration: none; }
.addp { display: inline-flex; align-items: center; gap: 4px; }
.addp .sel { background: #14161a; color: #d0d0d0; border: 1px solid #555; }
.addp .il { width: 120px; background: #14161a; color: #d0d0d0; border: 1px solid #555; padding: 2px 4px; }
.addp .ip { width: 56px; background: #14161a; color: #d0d0d0; border: 1px solid #555; padding: 2px 4px; }
.rubric-edit { margin-top: 6px; display: flex; align-items: flex-start; gap: 8px; font-size: 12px; color: #b8bdc4; }
.rubric-edit .pl { color: #9aa0a6; padding-top: 4px; white-space: nowrap; }
.rubric-edit .rb { flex: 1; min-height: 54px; resize: vertical; background: #14161a; color: #d0d0d0;
  border: 1px solid #555; padding: 4px 6px; font-family: ui-monospace, monospace; font-size: 12px; }
</style>
