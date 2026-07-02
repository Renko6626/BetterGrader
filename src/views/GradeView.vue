<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { NModal } from "naive-ui";
import { buildQueue, listPresets, setScore, studentPages } from "../api";
import type { GradingUnit, Preset, PageRef } from "../types";
import {
  initialGradeState, reduceGradeKey,
  type GradeState, type GradeCtx, type GradeEffect,
} from "../composables/useGradeKeys";

// M1：固定用 seed 出来的 exam_id=1、先批第 1 题（真实选题在后续计划）
const examId = 1;
const problemNumber = ref(1);

const queue = ref<GradingUnit[]>([]);
const presets = ref<Preset[]>([]);
const gs = ref<GradeState>(initialGradeState());
const peekPages = ref<PageRef[]>([]);   // 当前学生全部页，供轴2 速览

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

async function loadQueue() {
  queue.value = await buildQueue(examId, problemNumber.value);
  presets.value = current.value ? await listPresets(current.value.problem_id) : [];
  await refreshPeek();
}
async function refreshPeek() {
  peekPages.value = current.value ? await studentPages(current.value.student_id) : [];
}

async function applyEffect(eff: GradeEffect) {
  const u = current.value;
  if (!u) return;
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
      break;
    case "nextFlag": {
      const next = queue.value.findIndex((x, i) => i > gs.value.index && x.state === "Flagged");
      if (next >= 0) { gs.value = { ...gs.value, index: next, peek: 0, manual: false, buffer: "" }; await refreshPeek(); }
      break;
    }
    case "jump":
      gs.value = { ...gs.value, index: eff.index, peek: 0, manual: false, buffer: "", overview: false };
      await refreshPeek();
      break;
    case "none": break;
  }
}

function onKey(e: KeyboardEvent) {
  const r = reduceGradeKey(gs.value, e.key, ctx.value);
  gs.value = r.state;
  e.preventDefault();
  applyEffect(r.effect);
}

function jumpFromOverview(i: number) {
  applyEffect({ kind: "jump", index: i });
}

onMounted(() => { window.addEventListener("keydown", onKey); loadQueue(); });
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="grade" v-if="current">
    <div class="pane">
      <div class="img">
        <img v-if="isRealImage(shownImage)" :src="shownImage!" alt="答卷" />
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
      </aside>
    </div>
    <footer>
      本题进度 {{ gs.index + 1 }} / {{ queue.length }}
      ｜[F]存疑 [J]下一存疑 [G]总览 [M/0]手动 [←/→]速览
    </footer>

    <!-- 队列总览（G 打开） -->
    <n-modal :show="gs.overview" @update:show="(v: boolean) => { if (!v) gs.overview = false; }">
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
  <section v-else class="grade"><p>队列为空。先到"考试设置"点"载入假考试"。</p></section>
</template>

<style scoped>
.grade { height: 100vh; display: flex; flex-direction: column; font-family: ui-monospace, monospace; color: #d0d0d0; background: #14161a; }
.pane { flex: 1; display: flex; }
.img { flex: 1; display: flex; align-items: center; justify-content: center; }
.img img { max-width: 100%; max-height: 100%; }
.placeholder { border: 1px dashed #555; padding: 40px; text-align: center; color: #888; }
.panel { width: 260px; border-left: 1px solid #333; padding: 12px; }
.panel li { list-style: none; margin: 2px 0; }
.total { margin-top: 12px; font-size: 18px; }
.manual { color: #7fd; }
footer { border-top: 1px solid #333; padding: 8px 12px; font-size: 13px; }
.overview { width: 480px; max-height: 70vh; background: #1c1f24; border: 1px solid #444; padding: 16px; overflow: auto; font-family: ui-monospace, monospace; color: #d0d0d0; }
.overview li { cursor: pointer; padding: 2px 0; list-style: none; }
.overview li.cur { color: #7fd; }
</style>
