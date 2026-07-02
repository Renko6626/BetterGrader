<script setup lang="ts">
import { ref, onMounted } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { newExam, openExam, seedDemoExam, currentExam, listProblems, listPresets, listStudents } from "../api";
import type { Problem, Preset, Student, ExamInfo } from "../types";
import { NButton, NCard, NDataTable, NAlert, NSpace, NTag } from "naive-ui";

const exam = ref<ExamInfo | null>(null);
const problems = ref<Problem[]>([]);
const presetsByProblem = ref<Record<number, Preset[]>>({});
const students = ref<Student[]>([]);
const errorMsg = ref("");

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
onMounted(refresh);
</script>

<template>
  <section style="padding:16px; font-family:ui-monospace,monospace;">
    <n-space>
      <n-button @click="doNew">新建考试…</n-button>
      <n-button @click="doOpen">打开考试…</n-button>
      <n-button type="primary" @click="doDemo">新建演示考试…</n-button>
    </n-space>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px" />
    <p v-if="exam">当前：{{ exam.name }}</p>
    <p v-else>未打开考试。点上面按钮选一个目录。</p>

    <div v-for="p in problems" :key="p.id" style="margin:8px 0;">
      <h3>题{{ p.number }} · {{ p.title }}
        <n-tag size="small">满分 {{ p.max_score }}</n-tag>
      </h3>
      <ul><li v-for="pr in presetsByProblem[p.id]" :key="pr.id">键 {{ pr.slot }} → {{ pr.label }} = {{ pr.points }}</li></ul>
    </div>
    <n-card v-if="students.length" :title="`花名册（${students.length} 人）`">
      <n-data-table :columns="[{title:'姓名',key:'name'},{title:'考号',key:'exam_number'}]" :data="students" />
    </n-card>
  </section>
</template>
