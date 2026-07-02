<script setup lang="ts">
import { ref, onMounted, h } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  newExam, openExam, seedDemoExam, currentExam, listProblems, listPresets, listStudents, ingestFolder,
  listPdfs, readPdf, savePdfPage, addStudent, renameStudent, deleteStudent,
} from "../api";
import type { Problem, Preset, Student, ExamInfo } from "../types";
import { NButton, NCard, NDataTable, NAlert, NSpace, NTag } from "naive-ui";
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

async function refresh() {
  exam.value = await currentExam();
  if (!exam.value) { problems.value = []; students.value = []; presetsByProblem.value = {}; return; }
  problems.value = await listProblems();
  const np: Record<number, Preset[]> = {};
  for (const p of problems.value) np[p.id] = await listPresets(p.id);
  presetsByProblem.value = np;
  students.value = await listStudents();
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
  errorMsg.value = "";
  ingestMsg.value = "";
  try {
    const dir = await open({ directory: true, multiple: false, title: "选择图片文件夹" });
    if (typeof dir !== "string") return;
    const n = await ingestFolder(dir);
    ingestMsg.value = `已导入 ${n} 张图（去"标注"页开始标注）`;
  } catch (e) { errorMsg.value = String(e); }
}
async function doImportPdfs() {
  pdfMsg.value = ""; errorMsg.value = "";
  try {
    const dir = await open({ directory: true, multiple: false, title: "选择 PDF 文件夹" });
    if (typeof dir !== "string") return;
    importing.value = true;
    const pdfs = await listPdfs(dir);
    let done = 0; const failed: string[] = [];
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
      pdfMsg.value = `已导入 ${done}/${pdfs.length}…`;
    }
    pdfMsg.value = failed.length
      ? `完成：导入 ${done} 份；失败 ${failed.length} 份（${failed.join("、")}）——可重试这些文件`
      : `完成：导入 ${done} 份 PDF（去"判分"直接开批；页数不符的在"标注"确认总表里修）`;
    await refresh();
  } catch (e) { errorMsg.value = String(e); } finally { importing.value = false; }
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
    </n-space>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px" />
    <n-alert v-if="ingestMsg" type="success" :title="ingestMsg" closable @close="ingestMsg=''" style="margin-top:8px" />
    <n-alert v-if="pdfMsg" type="success" :title="pdfMsg" closable @close="pdfMsg=''" style="margin-top:8px" />
    <p v-if="exam">当前：{{ exam.name }}</p>
    <p v-else>未打开考试。点上面按钮选一个目录。</p>

    <div v-for="p in problems" :key="p.id" style="margin:8px 0;">
      <h3>题{{ p.number }} · {{ p.title }}
        <n-tag size="small">满分 {{ p.max_score }}</n-tag>
      </h3>
      <ul><li v-for="pr in presetsByProblem[p.id]" :key="pr.id">键 {{ pr.slot }} → {{ pr.label }} = {{ pr.points }}</li></ul>
    </div>
    <n-card v-if="students.length" :title="`花名册（${students.length} 人）`">
      <n-data-table :columns="studentColumns" :data="students" :row-key="(row) => row.id" />
    </n-card>
  </section>
</template>
