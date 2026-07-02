<script setup lang="ts">
import { ref, h } from "vue";
import { NSpace, NButton, NCard, NTag, NText, NAlert, NDataTable } from "naive-ui";
import type { DataTableColumns } from "naive-ui";
import { seedFake, listProblems, listPresets, listStudents } from "../api";
import type { Problem, Preset, Student } from "../types";

const examId = ref<number | null>(null);
const problems = ref<Problem[]>([]);
const presetsByProblem = ref<Record<number, Preset[]>>({});
const students = ref<Student[]>([]);
const loading = ref(false);
const errorMsg = ref<string | null>(null);

const presetColumns: DataTableColumns<Preset> = [
  { title: "键", key: "slot", width: 80 },
  { title: "档位", key: "label" },
  { title: "分值", key: "points", width: 100, render: (row) => h("span", `${row.points} 分`) },
];

const studentColumns: DataTableColumns<Student> = [
  { title: "序号", key: "roster_order", width: 80, render: (row) => row.roster_order ?? "—" },
  { title: "姓名", key: "name" },
  { title: "考号", key: "exam_number", render: (row) => row.exam_number ?? "—" },
];

async function seed() {
  loading.value = true;
  errorMsg.value = null;
  examId.value = null;
  problems.value = [];
  presetsByProblem.value = {};
  students.value = [];
  try {
    examId.value = await seedFake();
    problems.value = await listProblems(examId.value);
    const nextPresets: Record<number, Preset[]> = {};
    for (const p of problems.value) nextPresets[p.id] = await listPresets(p.id);
    presetsByProblem.value = nextPresets;
    students.value = await listStudents(examId.value);
  } catch (err) {
    errorMsg.value = err instanceof Error ? err.message : String(err);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <n-space vertical size="large" class="setup">
    <n-space align="center">
      <n-button type="primary" :loading="loading" @click="seed">载入假考试（M1 验收数据）</n-button>
      <n-text v-if="examId !== null" depth="3">exam_id = {{ examId }}（判分视图会用这场）</n-text>
    </n-space>

    <n-alert v-if="errorMsg" type="error" title="IPC 调用失败" closable @close="errorMsg = null">
      {{ errorMsg }}
    </n-alert>

    <n-space vertical size="medium" v-if="problems.length" item-style="width: 100%">
      <n-card v-for="p in problems" :key="p.id" size="small" :title="`题${p.number} · ${p.title}`">
        <template #header-extra>
          <n-tag type="info" size="small" round>满分 {{ p.max_score }}</n-tag>
        </template>
        <n-data-table :columns="presetColumns" :data="presetsByProblem[p.id] || []" :bordered="false" size="small" />
      </n-card>
    </n-space>

    <n-card v-if="students.length" size="small" :title="`花名册（${students.length} 人）`">
      <n-data-table :columns="studentColumns" :data="students" :bordered="false" size="small" />
    </n-card>
  </n-space>
</template>

<style scoped>
.setup {
  padding: 16px;
  font-family: ui-monospace, monospace;
}
</style>
