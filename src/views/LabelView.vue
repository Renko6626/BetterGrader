<!-- src/views/LabelView.vue -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { listPages, setPageLabel, labelingSummary, listStudents, addStudent } from "../api";
import type { PageRow, LabelSummary, Student } from "../types";
import { useImage } from "../composables/useImage";
import { initialLabelState, reduceLabelKey, pickStudent, type LabelState, type LabelEffect } from "../composables/useLabelKeys";
import { NModal, NInput, NButton, NAlert } from "naive-ui";

const pages = ref<PageRow[]>([]);
const students = ref<Student[]>([]);
const summary = ref<LabelSummary | null>(null);
const ls = ref<LabelState>(initialLabelState());
const { url, show } = useImage();
const errorMsg = ref("");
const pickQuery = ref("");
const problemInput = ref("");      // 「只切题」：直接改本页题号的输入框
const problemFocused = ref(false); // 输入框获焦时放行键盘，不进 reducer

const cur = computed(() => pages.value[ls.value.index] ?? null);
const ctx = computed(() => ({ pageCount: pages.value.length }));
const filteredStudents = computed(() => {
  const q = pickQuery.value.trim().toLowerCase();
  if (!q) return students.value;
  return students.value.filter(s => s.name.toLowerCase().includes(q) || (s.exam_number ?? "").toLowerCase().includes(q));
});

const studentNameOf = (id: number | null) =>
  id == null ? null : (students.value.find(s => s.id === id)?.name ?? `#${id}`);
// 本页真实归属（读已标状态，不是 reducer 的手动流状态）
const curLabel = computed(() => {
  const c = cur.value;
  if (!c) return "—";
  const who = studentNameOf(c.student_id) ?? "未绑定";
  const q = c.problem_number == null ? "未标注" : (c.problem_number === 0 ? "姓名页" : `题${c.problem_number}`);
  return `${who} · ${q}`;
});
// 翻页/加载后：把题号输入框同步成本页真实题号；本页若已绑学生，让手动流从这里续
function syncPageEdit() {
  const c = cur.value;
  problemInput.value = c && c.problem_number != null ? String(c.problem_number) : "";
  if (c && c.student_id != null) {
    ls.value = { ...ls.value, currentStudent: c.student_id, nextProblem: (c.problem_number ?? 0) + 1 };
  }
}
// 只切题：保留本页已绑的学生，仅改题号（0=姓名页）；不改学生
async function applyProblemInput() {
  const c = cur.value;
  if (!c) return;
  const t = problemInput.value.trim();
  if (t === "") return;
  const n = parseInt(t, 10);
  if (Number.isNaN(n) || n < 0) { errorMsg.value = "题号需为 ≥0 的整数（0=姓名页）"; return; }
  if (c.student_id == null) { errorMsg.value = "本页还没绑定学生——先用 S 选人，再改题号"; return; }
  try {
    await setPageLabel(c.id, c.student_id, n);
    c.problem_number = n; c.status = "labeled";
    errorMsg.value = "";
  } catch (e) { errorMsg.value = String(e); }
}

async function reload() {
  try {
    pages.value = await listPages();
    students.value = await listStudents();
    await refreshImage();
  } catch (e) {
    errorMsg.value = String(e);
  }
}
async function refreshImage() {
  try {
    await show(cur.value ? cur.value.image_path : null);
  } catch (e) {
    errorMsg.value = String(e);
  }
  syncPageEdit(); // 每次换页同步题号输入框 + 手动流续接
}
async function refreshSummary() {
  try {
    summary.value = await labelingSummary();
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function applyEffect(eff: LabelEffect, targetPage: PageRow | null) {
  errorMsg.value = ""; // 每次尝试先清掉旧的失败横幅
  if (eff.kind === "assign" && targetPage) {
    try {
      await setPageLabel(targetPage.id, eff.studentId, eff.problemNumber);
      // 本地同步，避免整列重查
      targetPage.student_id = eff.studentId; targetPage.problem_number = eff.problemNumber; targetPage.status = "labeled";
    } catch (e) {
      errorMsg.value = String(e);
    }
  }
}

async function onKey(e: KeyboardEvent) {
  if (problemFocused.value) return; // 改题号输入框获焦时放行，别把打字当判分/标注键
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  if (ls.value.picker) return; // 选人态交给 NModal 输入，不进 reducer
  const before = ls.value.index;
  const targetPage = cur.value; // 落库目标必须是按键时屏幕上的页——在 ls.value 前进前捕获
  const r = reduceLabelKey(ls.value, e.key, ctx.value);
  const handled = r.effect.kind !== "none" || JSON.stringify(r.state) !== JSON.stringify(ls.value);
  ls.value = r.state;
  if (handled) e.preventDefault(); // 只吞掉 reducer 真正消费的键
  await applyEffect(r.effect, targetPage);
  if (ls.value.index !== before) await refreshImage();
}

async function confirmPick(studentId: number) {
  const targetPage = cur.value; // 当前正在看的姓名页——在 ls.value 前进前捕获
  const r = pickStudent(ls.value, studentId, ctx.value);
  ls.value = r.state;
  pickQuery.value = "";
  await applyEffect(r.effect, targetPage);
  await refreshImage();
}
async function addAndPick() {
  const name = pickQuery.value.trim();
  if (!name) return;
  try {
    const id = await addStudent(name, null); // 临时新增（无考号）
    students.value = await listStudents();
    await confirmPick(id);
  } catch (e) {
    errorMsg.value = String(e);
  }
}

onMounted(() => { window.addEventListener("keydown", onKey); reload(); });
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="label">
    <n-alert v-if="errorMsg" type="error" :title="errorMsg" closable @close="errorMsg=''" />
    <div v-if="!pages.length" class="empty">没有图片。先在“考试设置”导入图片文件夹。</div>
    <template v-else>
      <div class="pane">
        <div class="img">
          <img v-if="url" :src="url" alt="答卷" />
          <div v-else class="ph">（无图/加载中）</div>
        </div>
        <aside class="side">
          <p>第 {{ ls.index + 1 }} / {{ pages.length }} 张</p>
          <p class="cur">本页归属：<b>{{ curLabel }}</b></p>
          <div class="edit">
            <label>改本页题号
              <input class="pn" v-model="problemInput" inputmode="numeric"
                     @focus="problemFocused = true"
                     @blur="problemFocused = false; applyProblemInput()"
                     @keyup.enter="applyProblemInput()" />
            </label>
            <span class="hint">0=姓名页，回车生效，学生不变</span>
          </div>
          <hr class="sep" />
          <p class="dim">手动流：当前学生 {{ studentNameOf(ls.currentStudent) ?? "—" }}｜下一题 {{ ls.nextProblem }}</p>
          <p class="keys">[S]姓名页/选人 [Enter]派题 [C]接上题 [N]跳题 [←→]翻页</p>
          <n-button size="small" @click="refreshSummary">刷新确认总表</n-button>
        </aside>
      </div>

      <!-- 花名册选人 -->
      <n-modal v-model:show="ls.picker" :close-on-esc="true" preset="card" title="选人（搜姓名/键考号）" style="width:420px">
        <n-input v-model:value="pickQuery" placeholder="姓名或考号" autofocus @keyup.enter="filteredStudents[0] && confirmPick(filteredStudents[0].id)" />
        <ul class="picklist">
          <li v-for="s in filteredStudents" :key="s.id" @click="confirmPick(s.id)">{{ s.name }}（{{ s.exam_number ?? "—" }}）</li>
        </ul>
        <n-button size="small" @click="addAndPick">＋ 名册没有，临时新增「{{ pickQuery }}」</n-button>
      </n-modal>

      <!-- 确认总表 -->
      <div v-if="summary" class="summary">
        <h3>确认总表（未标注 {{ summary.unlabeled_pages }} 张）</h3>
        <table>
          <thead><tr><th>学生</th><th>答题页</th><th>题数N</th><th>校验</th></tr></thead>
          <tbody>
            <tr v-for="st in summary.stacks" :key="st.student_id" :class="{ bad: !st.count_ok }">
              <td>{{ st.student_name }}</td><td>{{ st.answer_pages }}</td><td>{{ st.problem_count }}</td>
              <td>{{ st.count_ok ? "✓" : "✗ 页数不符，逐页指认" }}</td>
            </tr>
          </tbody>
        </table>
        <p v-if="summary.absent_students.length" class="absent">
          缺考（花名册有、无卷）：{{ summary.absent_students.map(s => s.name).join("、") }}
        </p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.label { height: 100%; display: flex; flex-direction: column; font-family: ui-monospace, monospace; color: #d0d0d0; }
.pane { flex: 1; display: flex; min-height: 0; }
.img { flex: 1; display: flex; align-items: center; justify-content: center; overflow: auto; }
.img img { max-width: 100%; max-height: 100%; }
.ph { border: 1px dashed #555; padding: 40px; color: #888; }
.side { width: 260px; border-left: 1px solid #333; padding: 12px; }
.side .cur { font-size: 15px; }
.side .cur b { color: #7fd; }
.side .edit { margin: 8px 0; }
.side .edit label { display: inline-flex; align-items: center; gap: 6px; }
.side .pn { width: 56px; background: #14161a; border: 1px solid #555; color: #d0d0d0;
  font-family: inherit; padding: 3px 6px; text-align: center; }
.side .hint { display: block; color: #888; font-size: 11px; margin-top: 4px; }
.side .sep { border: none; border-top: 1px solid #2a2d33; margin: 12px 0; }
.side .dim { color: #9aa0a6; font-size: 12px; }
.side .keys { color: #888; font-size: 12px; margin-top: 12px; }
.picklist { max-height: 220px; overflow: auto; margin: 8px 0; }
.picklist li { cursor: pointer; padding: 2px 4px; list-style: none; }
.picklist li:hover { color: #7fd; }
.summary { border-top: 1px solid #333; padding: 8px 12px; max-height: 30vh; overflow: auto; }
.summary table { border-collapse: collapse; }
.summary th, .summary td { border: 1px solid #444; padding: 2px 10px; }
.summary tr.bad td { color: #f77; }
.summary .absent { color: #fb7; }
</style>
