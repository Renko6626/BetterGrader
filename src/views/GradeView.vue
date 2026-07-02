<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { NModal, NAlert } from "naive-ui";
import { buildQueue, listPresets, setScore, studentPages, listProblems, setComment } from "../api";
import type { GradingUnit, Preset, PageRef, Problem } from "../types";
import { useImage } from "../composables/useImage";
import {
  initialGradeState, reduceGradeKey,
  type GradeState, type GradeCtx, type GradeEffect,
} from "../composables/useGradeKeys";

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

const current = computed(() => queue.value[gs.value.index]);
const ctx = computed<GradeCtx>(() => ({
  queueLength: queue.value.length,
  peekMin: -(Math.max(peekPages.value.length - 1, 0)),
  peekMax: Math.max(peekPages.value.length - 1, 0),
}));

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
    syncComment();
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
        syncComment();
        break;
      case "nextFlag": {
        const next = queue.value.findIndex((x, i) => i > gs.value.index && x.state === "Flagged");
        if (next >= 0) { gs.value = { ...gs.value, index: next, peek: 0, manual: false, buffer: "" }; await refreshPeek(); syncComment(); }
        break;
      }
      case "jump":
        gs.value = { ...gs.value, index: eff.index, peek: 0, manual: false, buffer: "", overview: false };
        await refreshPeek();
        syncComment();
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

// 速览偏移/换单元时 shownImage 变化 → 刷新真图（换题时 loadQueue 已显式刷新一次）
watch(shownImage, () => { refreshImg(); });

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await initProblems();
  await loadQueue();
});
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="grade" v-if="current">
    <header class="picker">
      题目：
      <button v-for="p in problems" :key="p.id" :class="{ on: p.number === problemNumber }"
              @click="onSelectProblem(p.number)">题{{ p.number }}</button>
    </header>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg = ''" />
    <div class="pane">
      <div class="img">
        <img v-if="imgUrl" :src="imgUrl" alt="答卷" />
        <div v-else class="placeholder">
          <div>占位图（无真实扫描）</div>
          <div>{{ current.student_name }} · 题{{ problemNumber }}
            <span v-if="gs.peek !== 0">· 速览偏移 {{ gs.peek }}</span>
          </div>
        </div>
      </div>
      <aside class="panel">
        <h3>题{{ current.problem_number }}</h3>
        <ul>
          <li v-for="p in presets" :key="p.id">
            <b>{{ p.slot }}</b> {{ p.label }} {{ p.points }}
          </li>
        </ul>
        <p class="total">当前：{{ current.total ?? "—" }}｜{{ current.state }}</p>
        <p v-if="gs.manual" class="manual">手动输入：{{ gs.buffer || "_" }}（Enter 确认）</p>
        <div class="comment">
          <label>评语</label>
          <textarea v-model="commentText" placeholder="本题评语（可选）"
            @focus="commentFocused = true"
            @blur="commentFocused = false; saveCurrentComment()"></textarea>
        </div>
      </aside>
    </div>
    <footer>
      本题进度 {{ gs.index + 1 }} / {{ queue.length }}
      ｜[F]存疑 [J]下一存疑 [G]总览 [M/0]手动 [←/→]速览
    </footer>

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
      题目：
      <button v-for="p in problems" :key="p.id" :class="{ on: p.number === problemNumber }"
              @click="onSelectProblem(p.number)">题{{ p.number }}</button>
    </header>
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg = ''" />
    <p>队列为空。先到"考试设置"打开或新建一场考试，或导入图片文件夹并标注。</p>
  </section>
</template>

<style scoped>
.grade { height: 100%; display: flex; flex-direction: column; font-family: ui-monospace, monospace; color: #d0d0d0; background: #14161a; }
.picker { border-bottom: 1px solid #333; padding: 8px 12px; font-size: 13px; }
.picker button { background: none; border: 1px solid #444; color: #d0d0d0; padding: 2px 10px; margin-left: 6px; cursor: pointer; font-family: inherit; }
.picker button.on { border-color: #7fd; color: #7fd; }
.pane { flex: 1; display: flex; }
.img { flex: 1; display: flex; align-items: center; justify-content: center; }
.img img { max-width: 100%; max-height: 100%; }
.placeholder { border: 1px dashed #555; padding: 40px; text-align: center; color: #888; }
.panel { width: 260px; border-left: 1px solid #333; padding: 12px; }
.panel li { list-style: none; margin: 2px 0; }
.total { margin-top: 12px; font-size: 18px; }
.manual { color: #7fd; }
.comment { margin-top: 16px; display: flex; flex-direction: column; gap: 4px; }
.comment label { font-size: 12px; color: #888; }
.comment textarea {
  width: 100%; min-height: 72px; resize: vertical; box-sizing: border-box;
  background: #1c1f24; color: #d0d0d0; border: 1px solid #444; padding: 6px;
  font-family: inherit; font-size: 13px;
}
footer { border-top: 1px solid #333; padding: 8px 12px; font-size: 13px; }
.overview { width: 480px; max-height: 70vh; background: #1c1f24; border: 1px solid #444; padding: 16px; overflow: auto; font-family: ui-monospace, monospace; color: #d0d0d0; }
.overview li { cursor: pointer; padding: 2px 0; list-style: none; }
.overview li.cur { color: #7fd; }
</style>
