<!-- src/views/ExportView.vue -->
<script setup lang="ts">
import { ref, computed } from "vue";
import { save, open } from "@tauri-apps/plugin-dialog";
import { PDFDocument } from "pdf-lib";
import { exportSummary, saveCsv, listStudents, studentPages, readImage, saveExportFile } from "../api";
import type { ExportData, Student } from "../types";
import { NButton, NAlert, NSpace, NProgress, NCheckbox, NRadioGroup, NRadioButton } from "naive-ui";
import { humanizeError, isNoExam } from "../errors";

const data = ref<ExportData | null>(null);
const errorMsg = ref("");
const noExam = ref(false); // 当前报错是否为"未打开考试"——决定用引导(warning)还是报错(error)样式
const okMsg = ref("");
const tried = ref(false); // 是否已尝试过计算（区分"从未点过"与"点过但无考试"）
const includeComments = ref(false); // 导出 CSV 时是否含每题评语列
const nameMode = ref<"name" | "number">("name"); // 每人 PDF 文件名取姓名还是学号
const exporting = ref(false);
const prog = ref<{ done: number; total: number } | null>(null);
const progPct = computed(() =>
  prog.value && prog.value.total > 0 ? Math.round(prog.value.done / prog.value.total * 100) : 0);
const paintTick = () => new Promise<void>((r) => setTimeout(r, 0)); // 让出一帧，进度条才会动

function reportError(e: unknown) { noExam.value = isNoExam(e); errorMsg.value = humanizeError(e); }
async function load() {
  errorMsg.value = ""; okMsg.value = ""; tried.value = true;
  try { data.value = await exportSummary(); }
  catch (e) { data.value = null; reportError(e); }
}
async function doSaveCsv() {
  errorMsg.value = ""; okMsg.value = "";
  try {
    const path = await save({ title: "保存成绩表 CSV", defaultPath: "成绩表.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    await saveCsv(path, includeComments.value);
    okMsg.value = `已保存：${path}`;
  } catch (e) { reportError(e); }
}
function printReport() { window.print(); }

// 文件名清洗：去掉 Windows/护栏非法字符与 .. 与尾部点/空格；空则兜底 unnamed
function sanitizeName(raw: string): string {
  const s = raw.replace(/[\\/:*?"<>|]/g, "_").replace(/\.\.+/g, "_").replace(/[\s.]+$/, "").trim();
  return s || "unnamed";
}
// 选姓名/学号派生文件名主干；学号缺失回退姓名；重名自动加 _2/_3 后缀防覆盖
function uniqueBase(s: Student, used: Set<string>): string {
  const raw = nameMode.value === "number" ? (s.exam_number || s.name) : s.name;
  const base = sanitizeName(raw);
  let cand = base, i = 2;
  while (used.has(cand.toLowerCase())) cand = `${base}_${i++}`;
  used.add(cand.toLowerCase());
  return cand;
}

// 把每个考生的图片按扫描序（含姓名页）拼成一份 PDF，写到用户选的目录。
async function doExportPdfs() {
  errorMsg.value = ""; okMsg.value = "";
  const dir = await open({ directory: true, multiple: false, title: "选择 PDF 输出目录" });
  if (typeof dir !== "string") return;
  exporting.value = true;
  try {
    const students = await listStudents();
    const used = new Set<string>();
    let done = 0, skipped = 0, processed = 0;
    const failed: string[] = [];
    prog.value = { done: 0, total: students.length };
    for (const s of students) {
      // studentPages 按 seq 返回该生全部页（含姓名页 题0）；排除占位假图
      const pages = (await studentPages(s.id)).filter(p => p.image_path && !p.image_path.startsWith("fake://"));
      if (!pages.length) {
        skipped++; // 无卷（缺考）跳过——仍要推进进度，别让条卡住
      } else {
        try {
          const pdf = await PDFDocument.create();
          for (const pg of pages) {
            const bytes = new Uint8Array(await readImage(pg.image_path));
            const img = /\.png$/i.test(pg.image_path) ? await pdf.embedPng(bytes) : await pdf.embedJpg(bytes);
            const page = pdf.addPage([img.width, img.height]);
            page.drawImage(img, { x: 0, y: 0, width: img.width, height: img.height });
          }
          const out = await pdf.save();
          await saveExportFile(dir, uniqueBase(s, used) + ".pdf", Array.from(out));
          done++;
        } catch { failed.push(s.name); }
      }
      prog.value = { done: ++processed, total: students.length };
      await paintTick(); // 让进度条真正刷新，别让 pdf-lib 霸着主线程
    }
    okMsg.value = `已导出 ${done} 份 PDF 到 ${dir}`
      + (skipped ? `；跳过 ${skipped} 个无卷学生` : "")
      + (failed.length ? `；失败 ${failed.length} 个（${failed.join("、")}）` : "");
  } catch (e) { reportError(e); }
  finally { exporting.value = false; prog.value = null; }
}

// 展示用：单元文本（与 CSV 规则一致）
function cellText(r: ExportData["rows"][number], i: number): string {
  if (r.absent) return "缺考";
  const c = r.cells[i];
  if (c.state === "Graded") return c.total?.toString() ?? "";
  if (c.state === "Flagged") return c.total != null ? `${c.total}?` : "?";
  return ""; // Ungraded 留空
}

</script>

<template>
  <section class="export">
    <n-space>
      <n-button @click="load">计算成绩汇总</n-button>
      <n-button v-if="data" @click="doSaveCsv">保存 CSV…</n-button>
      <n-button v-if="data" @click="printReport">打印 / 导出 PDF</n-button>
      <n-checkbox v-if="data" v-model:checked="includeComments">含每题评语列</n-checkbox>
      <n-button :loading="exporting" :disabled="exporting" @click="doExportPdfs">导出每人 PDF…</n-button>
      <span class="namemode">文件名
        <n-radio-group v-model:value="nameMode" size="small">
          <n-radio-button value="name">姓名</n-radio-button>
          <n-radio-button value="number">考号</n-radio-button>
        </n-radio-group>
      </span>
    </n-space>
    <div v-if="prog" class="prog-box">
      <span class="prog-label">导出每人 PDF… {{ prog.done }} / {{ prog.total }}</span>
      <n-progress type="line" :percentage="progPct" :indicator-placement="'inside'" />
    </div>
    <!-- 未打开考试 → 引导(warning)；其余 → 报错(error)。两者都由 humanizeError 转成中文可操作文案 -->
    <n-alert v-if="errorMsg && noExam" type="warning" title="尚未打开考试" :show-icon="true" style="margin-top:8px">
      {{ errorMsg }}
    </n-alert>
    <n-alert v-else-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" style="margin-top:8px"/>
    <n-alert v-if="okMsg" type="success" :title="okMsg" closable @close="okMsg=''" style="margin-top:8px"/>
    <p v-if="!tried && !errorMsg" style="margin-top:8px; opacity:0.8;">
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
.prog-box { margin-top: 10px; padding: 10px; border: 1px solid var(--border-accent); background: var(--elevated); }
.prog-label { display: block; font-size: 13px; color: var(--accent-soft); margin-bottom: 6px; }
.namemode { display: inline-flex; align-items: center; gap: 10px; font-size: 13px; }
.namemode label { display: inline-flex; align-items: center; gap: 3px; }
/* 打印时隐藏这些操作控件（沿用下方 @media print 规则里 n-space 已隐藏，这里补 alert） */
@media print { .namemode { display: none !important; } }
.coverage { margin: 12px 0; padding: 8px; border: 1px solid var(--border); }
.report table { border-collapse: collapse; margin: 8px 0; width: 100%; }
.report th, .report td { border: 1px solid var(--border); padding: 2px 8px; text-align: right; }
.report th:first-child, .report td:first-child { text-align: left; }
.report tr.absent { color: var(--text-faint); }
/* 打印：只留报表，去掉按钮/背景 */
@media print {
  .export > .n-space, .export > .n-alert, .coverage { display: none !important; }
  .report th, .report td { border-color: #000; color: #000; }
  .report, .report h2, .report h3 { color: #000 !important; }
}
</style>
