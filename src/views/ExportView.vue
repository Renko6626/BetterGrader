<!-- src/views/ExportView.vue -->
<script setup lang="ts">
import { ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { exportSummary, saveCsv } from "../api";
import type { ExportData } from "../types";
import { NButton, NAlert, NSpace } from "naive-ui";

const data = ref<ExportData | null>(null);
const errorMsg = ref("");
const okMsg = ref("");
const tried = ref(false); // 是否已尝试过计算（区分"从未点过"与"点过但无考试"）
const includeComments = ref(false); // 导出 CSV 时是否含每题评语列

async function load() {
  errorMsg.value = ""; okMsg.value = ""; tried.value = true;
  try { data.value = await exportSummary(); }
  catch (e) { data.value = null; errorMsg.value = String(e); }
}
async function doSaveCsv() {
  errorMsg.value = ""; okMsg.value = "";
  try {
    const path = await save({ title: "保存成绩表 CSV", defaultPath: "成绩表.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await saveCsv(path, includeComments.value);
    okMsg.value = `已保存：${path}`;
  } catch (e) { errorMsg.value = String(e); }
}
function printReport() { window.print(); }

// 展示用：单元文本（与 CSV 规则一致）
function cellText(r: ExportData["rows"][number], i: number): string {
  if (r.absent) return "缺考";
  const c = r.cells[i];
  if (c.state === "Graded") return c.total?.toString() ?? "";
  if (c.state === "Flagged") return c.total != null ? `${c.total}?` : "?";
  return ""; // Ungraded 留空
}

// "无考试"提示：exportSummary 报错文案里含 "no exam open" 时给出更清晰的中文引导
function isNoExamError(msg: string): boolean {
  return /no exam open/i.test(msg);
}
</script>

<template>
  <section class="export">
    <n-space>
      <n-button @click="load">计算成绩汇总</n-button>
      <n-button v-if="data" @click="doSaveCsv">保存 CSV…</n-button>
      <n-button v-if="data" @click="printReport">打印 / 导出 PDF</n-button>
      <label v-if="data" class="include-comments">
        <input type="checkbox" v-model="includeComments" /> 含每题评语列
      </label>
    </n-space>
    <n-alert v-if="errorMsg && !isNoExamError(errorMsg)" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px"/>
    <n-alert v-if="okMsg" type="success" :title="okMsg" closable @close="okMsg=''" style="margin-top:8px"/>

    <!-- 无考试打开时的清晰提示，替代空白/困惑的界面 -->
    <n-alert v-if="tried && !data && isNoExamError(errorMsg)" type="warning"
             title="尚未打开任何考试" style="margin-top:8px">
      请先在「考试设置」打开或新建一个考试，再回到这里计算成绩汇总。
    </n-alert>
    <p v-else-if="!tried" style="margin-top:8px; opacity:0.8;">
      点击"计算成绩汇总"生成覆盖率确认与可打印报表。
    </p>

    <!-- 覆盖率确认 -->
    <div v-if="data" class="coverage">
      花名册 {{ data.coverage.roster }} 人，缺考 {{ data.coverage.absent }}；
      单元 已判 {{ data.coverage.graded }} / 存疑 {{ data.coverage.flagged }} / 未判 {{ data.coverage.ungraded }}
      （共 {{ data.coverage.units_total }}）。
      <b v-if="!data.ranking_available">尚有未判/存疑 → 不出排名（先批完再排）。</b>
      <b v-else>全部已判 → 含排名。</b>
    </div>

    <!-- 可打印报表 -->
    <div v-if="data" id="report" class="report">
      <h2>{{ data.exam.name }} 成绩表</h2>
      <table>
        <thead><tr>
          <th>姓名</th><th>考号</th>
          <th v-for="n in data.problem_numbers" :key="n">题{{ n }}</th>
          <th>总分</th><th v-if="data.ranking_available">排名</th>
        </tr></thead>
        <tbody>
          <tr v-for="r in data.rows" :key="r.student_id" :class="{absent:r.absent}">
            <td>{{ r.name }}</td><td>{{ r.exam_number ?? "" }}</td>
            <td v-for="(_, i) in data.problem_numbers" :key="i">{{ cellText(r, i) }}</td>
            <td>{{ r.absent ? "缺考" : (r.total ?? "") }}</td>
            <td v-if="data.ranking_available">{{ r.rank ?? "" }}</td>
          </tr>
        </tbody>
      </table>
      <h3>各题得分率</h3>
      <table>
        <thead><tr><th>题</th><th>满分</th><th>平均</th><th>得分率</th><th>已评</th></tr></thead>
        <tbody>
          <tr v-for="ps in data.problem_stats" :key="ps.number">
            <td>题{{ ps.number }}</td><td>{{ ps.max_score }}</td>
            <td>{{ ps.avg != null ? ps.avg.toFixed(1) : "—" }}</td>
            <td>{{ ps.rate != null ? (ps.rate*100).toFixed(0)+"%" : "—" }}</td>
            <td>{{ ps.scored_count }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>

<style scoped>
.export { padding: 16px; font-family: ui-monospace, monospace; }
.include-comments { display: inline-flex; align-items: center; gap: 4px; font-size: 13px; }
.coverage { margin: 12px 0; padding: 8px; border: 1px solid #444; }
.report table { border-collapse: collapse; margin: 8px 0; width: 100%; }
.report th, .report td { border: 1px solid #555; padding: 2px 8px; text-align: right; }
.report th:first-child, .report td:first-child { text-align: left; }
.report tr.absent { color: #999; }
/* 打印：只留报表，去掉按钮/背景 */
@media print {
  .export > .n-space, .export > .n-alert, .coverage { display: none !important; }
  .report th, .report td { border-color: #000; color: #000; }
  .report, .report h2, .report h3 { color: #000 !important; }
}
</style>
