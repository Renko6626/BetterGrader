<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { NModal, NAlert } from "naive-ui";
import { buildQueue, listPresets, setScore, studentPages, listProblems, setComment, listStudents } from "../api";
import type { GradingUnit, Preset, PageRef, Problem } from "../types";
import { useImage } from "../composables/useImage";
import {
  initialGradeState, reduceGradeKey,
  type GradeState, type GradeCtx, type GradeEffect,
} from "../composables/useGradeKeys";
import GradeSidebar from "../components/GradeSidebar.vue";

// 题目选择：列出各题，用户选中后载该题队列（不再固定 problemNumber=1）
const problems = ref<Problem[]>([]);
const problemNumber = ref<number | null>(null);

const queue = ref<GradingUnit[]>([]);
const presets = ref<Preset[]>([]);
const gs = ref<GradeState>(initialGradeState());
const peekPages = ref<PageRef[]>([]);   // 当前学生全部页，供轴2 速览
const errorMsg = ref("");               // IPC 失败提示（落分失败必须可见）
const { url: imgUrl, show: showImg } = useImage();
const commentText = ref("");            // 当前单元评语（textarea 绑定）
const commentFocused = ref(false);      // 获焦时 onKey 必须放行，不当判分键处理

// 卷面缩放/平移（滚轮缩放、拖拽平移、双击复位）
const zoom = ref(1);
const panX = ref(0);
const panY = ref(0);
let dragging = false, lastX = 0, lastY = 0;
function onWheel(e: WheelEvent) {
  const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
  zoom.value = Math.min(8, Math.max(0.3, zoom.value * factor));
}
function onImgDown(e: MouseEvent) { dragging = true; lastX = e.clientX; lastY = e.clientY; }
function onImgMove(e: MouseEvent) {
  if (!dragging) return;
  panX.value += e.clientX - lastX; panY.value += e.clientY - lastY;
  lastX = e.clientX; lastY = e.clientY;
}
function onImgUp() { dragging = false; }
function resetZoom() { zoom.value = 1; panX.value = 0; panY.value = 0; }

const current = computed(() => queue.value[gs.value.index]);
const ctx = computed<GradeCtx>(() => ({
  queueLength: queue.value.length,
  peekMin: -(Math.max(peekPages.value.length - 1, 0)),
  peekMax: Math.max(peekPages.value.length - 1, 0),
}));
// 本题满分（面板标题用）与三态本地化
const curMax = computed(() => problems.value.find(p => p.number === problemNumber.value)?.max_score ?? "");
const stateLabel = (s: string) =>
  (({ Graded: "已判", Flagged: "存疑", Ungraded: "未判", Absent: "缺考" } as Record<string, string>)[s] ?? s);
// 本题进度计数
const counts = computed(() => {
  const c = { graded: 0, flagged: 0, ungraded: 0 };
  for (const u of queue.value) {
    if (u.state === "Graded") c.graded++;
    else if (u.state === "Flagged") c.flagged++;
    else c.ungraded++;
  }
  return c;
});
// 本题已判分数分布（实时直方图，校准标准漂移）
const dist = computed(() => {
  const max = Number(curMax.value) || 0;
  const perPoint = max > 0 && max <= 20;          // 满分≤20：每分一柱；否则 0..max 均分 11 柱
  const binCount = perPoint ? max + 1 : 11;
  const bins = Array.from({ length: binCount }, () => 0);
  let total = 0;
  for (const u of queue.value) {
    if ((u.state === "Graded" || u.state === "Flagged") && u.total != null) {
      const t = Math.max(0, Math.min(max, u.total));
      const idx = perPoint ? t : Math.min(binCount - 1, Math.round((t / (max || 1)) * (binCount - 1)));
      bins[idx]++; total++;
    }
  }
  return { bins, peak: Math.max(1, ...bins), perPoint, max, total, binCount };
});
// 本题平均分（已判/存疑单元），左侧面板顶部显示
const average = computed(() => {
  let sum = 0, n = 0;
  for (const u of queue.value) {
    if ((u.state === "Graded" || u.state === "Flagged") && u.total != null) { sum += u.total; n++; }
  }
  return n ? sum / n : null;
});
const sidebarCollapsed = ref(false); // 左侧悬浮面板收起态（Tab 切换）
// 考号映射（GradingUnit 只带姓名，考号从花名册取）
const examNoMap = ref<Record<number, string | null>>({});
const examNo = (sid: number) => examNoMap.value[sid] ?? null;
// 顶部进度条：已判/存疑各占一段
const pct = computed(() => {
  const n = queue.value.length || 1;
  return { g: counts.value.graded / n * 100, f: counts.value.flagged / n * 100 };
});

// 速览时显示哪张图：peek=0 → 当前 (学生,题号) 的首张；否则在该生全部页里偏移
const shownImage = computed(() => {
  if (!current.value) return null;
  if (gs.value.peek === 0) return current.value.pages[0] ?? null;
  const anchor = peekPages.value.findIndex(p => p.problem_number === problemNumber.value);
  const i = (anchor < 0 ? 0 : anchor) + gs.value.peek;
  return peekPages.value[i]?.image_path ?? null;
});
const isRealImage = (path: string | null) =>
  !!path && !path.startsWith("fake://"); // 真实文件用 convertFileSrc；此处仅判断是否占位

async function loadStudents() {
  try {
    const m: Record<number, string | null> = {};
    for (const s of await listStudents()) m[s.id] = s.exam_number;
    examNoMap.value = m;
  } catch { /* 考号只是展示，取不到不影响判分 */ }
}
async function initProblems() {
  try {
    problems.value = await listProblems();
    if (problems.value.length && problemNumber.value == null) problemNumber.value = problems.value[0].number;
  } catch (e) {
    errorMsg.value = String(e);
  }
}
async function loadQueue() {
  try {
    if (problemNumber.value == null) { queue.value = []; return; }
    queue.value = await buildQueue(problemNumber.value);
    presets.value = current.value ? await listPresets(current.value.problem_id) : [];
    await refreshPeek();
    await refreshImg();
    syncComment(); resetZoom();
  } catch (e) {
    errorMsg.value = String(e); // 未 seed 时 buildQueue 会拒绝，优雅降级为横幅+"队列为空"
  }
}
async function refreshPeek() {
  peekPages.value = current.value ? await studentPages(current.value.student_id) : [];
}
// 换单元后同步评语框显示为新单元的评语
function syncComment() {
  commentText.value = current.value?.comment ?? "";
}
// 评语框失焦时落库；成功后同步回本地 current.comment，避免下次读到旧值
async function saveCurrentComment() {
  const u = current.value;
  if (!u) return;
  const text = commentText.value; // 在 await 前捕获单元与文本，防止未来非受控切换写错单元
  try {
    await setComment(u.student_id, u.problem_id, text);
    (u as any).comment = text;
  } catch (e) {
    errorMsg.value = String(e);
  }
}
async function refreshImg() {
  // 真图：当前速览页或当前单元首图；fake:// 或缺失走占位
  try {
    const path = shownImage.value;
    await showImg(isRealImage(path) ? path : null);
  } catch (e) {
    errorMsg.value = String(e);
  }
}
function onSelectProblem(n: number) {
  if (n === problemNumber.value) return;
  problemNumber.value = n;
  gs.value = initialGradeState();
  loadQueue();
}

async function applyEffect(eff: GradeEffect) {
  const u = current.value;
  if (!u) return;
  errorMsg.value = ""; // 恢复动作先清掉旧的失败横幅
  try {
    switch (eff.kind) {
      case "setPreset": {
        const pr = presets.value.find(p => p.slot === eff.slot);
        if (!pr) return; // 该 slot 无档位，忽略
        await setScore(u.student_id, u.problem_id, pr.points, pr.id, "Graded");
        u.total = pr.points; u.state = "Graded"; u.preset_id = pr.id;
        break;
      }
      case "setManual":
        await setScore(u.student_id, u.problem_id, eff.value, null, "Graded");
        u.total = eff.value; u.state = "Graded"; u.preset_id = null;
        break;
      case "flag":
        await setScore(u.student_id, u.problem_id, u.total, u.preset_id, "Flagged");
        u.state = "Flagged";
        break;
      case "advance": case "back":
        presets.value = await listPresets(current.value!.problem_id);
        await refreshPeek();
        syncComment(); resetZoom();
        break;
      case "nextFlag": {
        const next = queue.value.findIndex((x, i) => i > gs.value.index && x.state === "Flagged");
        if (next >= 0) { gs.value = { ...gs.value, index: next, peek: 0, manual: false, buffer: "" }; await refreshPeek(); syncComment(); resetZoom(); }
        break;
      }
      case "jump":
        gs.value = { ...gs.value, index: eff.index, peek: 0, manual: false, buffer: "", overview: false };
        await refreshPeek();
        syncComment(); resetZoom();
        break;
      case "none": break;
    }
  } catch (e) {
    errorMsg.value = String(e);
  }
}

function onKey(e: KeyboardEvent) {
  if (commentFocused.value) return; // 评语框获焦时打字，绝不能被当成判分键拦截
  if (e.ctrlKey || e.metaKey || e.altKey) return; // 让 OS 快捷键（Ctrl+R/F、devtools 等）通过，绝不误触发落分
  if (e.key === "Tab") { e.preventDefault(); sidebarCollapsed.value = !sidebarCollapsed.value; return; } // Tab 收起/展开左侧面板（同时避免焦点移入评语框废掉判分键）
  const before = gs.value;
  const r = reduceGradeKey(gs.value, e.key, ctx.value);
  const handled = r.effect.kind !== "none"
    || JSON.stringify(r.state) !== JSON.stringify(before);
  gs.value = r.state;
  if (handled) e.preventDefault(); // 只吞掉 reducer 真正消费的键
  applyEffect(r.effect);
}

function jumpFromOverview(i: number) {
  applyEffect({ kind: "jump", index: i });
}
// 快速定位：从当前往后（循环）跳到下一个未判
function jumpToNextUngraded() {
  const n = queue.value.length;
  if (!n) return;
  for (let k = 1; k <= n; k++) {
    const i = (gs.value.index + k) % n;
    if (queue.value[i].state === "Ungraded") { applyEffect({ kind: "jump", index: i }); return; }
  }
}

// 速览偏移/换单元时 shownImage 变化 → 刷新真图（换题时 loadQueue 已显式刷新一次）
watch(shownImage, () => { refreshImg(); });

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await loadStudents();
  await initProblems();
  await loadQueue();
});
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="grade" v-if="current">
    <header class="picker">
      <div class="row">
        <span class="tabs">题目：
          <button v-for="p in problems" :key="p.id" :class="{ on: p.number === problemNumber }"
                  @click="onSelectProblem(p.number)">题{{ p.number }}</button>
        </span>
        <span class="who">
          <b class="nm">{{ current.student_name }}</b>
          <span v-if="examNo(current.student_id)" class="no">考号 {{ examNo(current.student_id) }}</span>
          <span class="pos">第 {{ gs.index + 1 }} / {{ queue.length }} 份</span>
          <span class="st" :class="current.state">{{ stateLabel(current.state) }}</span>
        </span>
      </div>
      <div class="row">
        <div class="pbar" :title="`已判 ${counts.graded}·存疑 ${counts.flagged}·未判 ${counts.ungraded} / 共 ${queue.length}`">
          <div class="seg g" :style="{ width: pct.g + '%' }"></div>
          <div class="seg f" :style="{ width: pct.f + '%' }"></div>
        </div>
        <span class="prog">
          已判 <b>{{ counts.graded }}</b> · 存疑 <b class="flg">{{ counts.flagged }}</b> · 未判 <b class="ung">{{ counts.ungraded }}</b> / 共 {{ queue.length }}
        </span>
        <button class="jump" :disabled="!counts.ungraded" @click="jumpToNextUngraded">下一个未判 →</button>
      </div>
    </header>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg = ''" />
    <div class="pane">
      <GradeSidebar :collapsed="sidebarCollapsed" :average="average" :dist="dist"
                    @toggle="sidebarCollapsed = !sidebarCollapsed" />
      <div class="img" @wheel.prevent="onWheel" @mousedown="onImgDown" @mousemove="onImgMove"
           @mouseup="onImgUp" @mouseleave="onImgUp" @dblclick="resetZoom">
        <img v-if="imgUrl" :src="imgUrl" alt="答卷" draggable="false"
             :style="{ transform: `translate(${panX}px,${panY}px) scale(${zoom})` }" />
        <div v-else class="placeholder">
          <div>占位图（无真实扫描）</div>
          <div>{{ current.student_name }} · 题{{ problemNumber }}
            <span v-if="gs.peek !== 0">· 速览偏移 {{ gs.peek }}</span>
          </div>
        </div>
      </div>
      <aside class="panel">
        <h3>题{{ current.problem_number }}<span v-if="curMax !== ''" class="mx">满分 {{ curMax }}</span></h3>
        <ul class="preset-list">
          <li v-for="p in presets" :key="p.id"><b>{{ p.slot }}</b> {{ p.label }} <span class="pt">{{ p.points }}</span></li>
        </ul>
        <p class="total">当前：{{ current.total ?? "—" }}｜{{ stateLabel(current.state) }}</p>
        <p v-if="gs.manual" class="manual">手动输入：{{ gs.buffer || "_" }}（Enter 确认）</p>
        <div class="comment">
          <label>评语</label>
          <textarea v-model="commentText" placeholder="本题评语（可选）"
            @focus="commentFocused = true"
            @blur="commentFocused = false; saveCurrentComment()"></textarea>
        </div>
        <div class="legend">
          <div class="lh">快捷键</div>
          <div><kbd>1–9</kbd> 档位给分　<kbd>M</kbd>/<kbd>0</kbd> 手动</div>
          <div><kbd>Enter</kbd> 下一份　<kbd>⌫ Backspace</kbd> 上一份</div>
          <div><kbd>←</kbd> <kbd>→</kbd> 速览邻页　<kbd>↓</kbd>/<kbd>Esc</kbd> 复位</div>
          <div><kbd>F</kbd> 存疑　<kbd>J</kbd> 下一存疑　<kbd>G</kbd> 总览</div>
          <div><kbd>Tab</kbd> 收起/展开左侧分布面板</div>
          <div class="dim">滚轮缩放 · 拖拽平移 · 双击复位</div>
        </div>
      </aside>
    </div>
    <!-- 队列总览（G 打开） -->
    <n-modal :show="gs.overview" :close-on-esc="false" :mask-closable="false"
             @update:show="(v: boolean) => { if (!v) gs.overview = false; }">
      <div class="overview">
        <h3>队列总览（键 Esc 关闭，点击跳转）</h3>
        <ol>
          <li v-for="(u, i) in queue" :key="u.student_id"
              :class="{ cur: i === gs.index }" @click="jumpFromOverview(i)">
            {{ i + 1 }}. {{ u.student_name }} — {{ u.state }}（{{ u.total ?? "—" }}）
          </li>
        </ol>
      </div>
    </n-modal>
  </section>
  <section v-else class="grade">
    <header class="picker">
      <span class="tabs">题目：
        <button v-for="p in problems" :key="p.id" :class="{ on: p.number === problemNumber }"
                @click="onSelectProblem(p.number)">题{{ p.number }}</button>
      </span>
    </header>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg = ''" />
    <p v-if="!problems.length">本场还没设置题目——先到"考试设置"用"生成题目"（填题数 + 每题满分）建题，才能判分。</p>
    <p v-else>本题队列为空：还没有卷子。先在"考试设置"导入 PDF/图片，并（PDF 会自动标注）在"标注"确认。</p>
  </section>
</template>

<style scoped>
.grade { height: 100%; display: flex; flex-direction: column; font-family: ui-monospace, monospace; color: #d0d0d0; background: #14161a; }
.picker { border-bottom: 1px solid #333; padding: 8px 12px; font-size: 13px; display: flex; flex-direction: column; gap: 8px; }
.picker .row { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.picker .tabs button { background: none; border: 1px solid #444; color: #d0d0d0; padding: 2px 10px; margin-left: 6px; cursor: pointer; font-family: inherit; }
.picker .tabs button.on { border-color: #7fd; color: #7fd; }
/* 当前这份是谁 */
.picker .who { display: flex; align-items: baseline; gap: 12px; margin-left: auto; white-space: nowrap; }
.picker .who .nm { color: #fff; font-size: 15px; }
.picker .who .no { color: #9aa0a6; font-size: 12px; }
.picker .who .pos { color: #b8bdc4; }
.picker .who .st { padding: 0 6px; border-radius: 3px; font-size: 12px; }
.picker .who .st.Graded { color: #7fd; }
.picker .who .st.Flagged { color: #fb7; }
.picker .who .st.Ungraded { color: #888; }
/* 进度条 */
.picker .pbar { flex: 1; min-width: 160px; height: 10px; background: #22262c; border: 1px solid #333; border-radius: 5px; overflow: hidden; display: flex; }
.picker .pbar .seg { height: 100%; transition: width .15s ease; }
.picker .pbar .seg.g { background: #4f8cff; }
.picker .pbar .seg.f { background: #d69138; }
.picker .prog { color: #b8bdc4; white-space: nowrap; }
.picker .prog b { color: #e6e6e6; }
.picker .prog .flg { color: #fb7; }
.picker .prog .ung { color: #fb7; }
.picker .jump { background: #22303f; border: 1px solid #3a5570; color: #cfe3ff; padding: 2px 10px; cursor: pointer; font-family: inherit; white-space: nowrap; }
.picker .jump:disabled { opacity: 0.4; cursor: default; }
.pane { flex: 1; display: flex; min-height: 0; position: relative; }
.img { flex: 1; display: flex; align-items: center; justify-content: center; overflow: hidden; cursor: grab; }
.img:active { cursor: grabbing; }
.img img { max-width: 100%; max-height: 100%; transform-origin: center center; user-select: none; will-change: transform; }
.placeholder { border: 1px dashed #555; padding: 40px; text-align: center; color: #888; }
.panel { width: 260px; border-left: 1px solid #333; padding: 12px; overflow: auto; }
.panel h3 .mx { color: #9aa0a6; font-size: 12px; font-weight: normal; margin-left: 8px; }
.preset-list { list-style: none; padding: 0; margin: 6px 0; }
.preset-list li { margin: 2px 0; }
.preset-list b { display: inline-block; width: 1.4em; color: #7fd; }
.preset-list .pt { color: #9aa0a6; }
.total { margin-top: 12px; font-size: 18px; }
.manual { color: #7fd; }
.legend { margin-top: 16px; border-top: 1px solid #2a2d33; padding-top: 10px; font-size: 12px; line-height: 1.9; color: #b8bdc4; }
.legend .lh { color: #9aa0a6; margin-bottom: 4px; }
.legend .dim { color: #888; margin-top: 4px; }
.legend kbd { background: #22262c; border: 1px solid #3a3f47; border-radius: 3px; padding: 0 5px; font-family: inherit; font-size: 11px; color: #e6e6e6; }
.comment { margin-top: 16px; display: flex; flex-direction: column; gap: 4px; }
.comment label { font-size: 12px; color: #888; }
.comment textarea {
  width: 100%; min-height: 72px; resize: vertical; box-sizing: border-box;
  background: #1c1f24; color: #d0d0d0; border: 1px solid #444; padding: 6px;
  font-family: inherit; font-size: 13px;
}
.overview { width: 480px; max-height: 70vh; background: #1c1f24; border: 1px solid #444; padding: 16px; overflow: auto; font-family: ui-monospace, monospace; color: #d0d0d0; }
.overview li { cursor: pointer; padding: 2px 0; list-style: none; }
.overview li.cur { color: #7fd; }
</style>
