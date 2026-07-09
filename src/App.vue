<script setup lang="ts">
import { ref } from "vue";
import { NConfigProvider, darkTheme } from "naive-ui";
import SetupView from "./views/SetupView.vue";
import LabelView from "./views/LabelView.vue";
import GradeView from "./views/GradeView.vue";
import ExportView from "./views/ExportView.vue";
const route = ref<"setup" | "label" | "grade" | "export">("setup");
</script>

<template>
  <n-config-provider :theme="darkTheme" class="app-shell">
    <nav class="topnav">
      <span class="brand">阅卷辅助器</span>
      <button :class="{ on: route === 'setup' }" @click="route = 'setup'"><i>1</i>考试设置</button>
      <button :class="{ on: route === 'label' }" @click="route = 'label'"><i>2</i>标注</button>
      <button :class="{ on: route === 'grade' }" @click="route = 'grade'"><i>3</i>判分</button>
      <button :class="{ on: route === 'export' }" @click="route = 'export'"><i>4</i>导出</button>
    </nav>
    <main class="app-main">
      <SetupView v-if="route === 'setup'" />
      <LabelView v-else-if="route === 'label'" />
      <GradeView v-else-if="route === 'grade'" />
      <ExportView v-else-if="route === 'export'" />
    </main>
  </n-config-provider>
</template>

<style>
/* ── 设计 token：全局(非 scoped)，所有视图/组件统一引用，杜绝 hex 漂移 ── */
:root {
  /* 表面：一条从底到上的三级坡道 */
  --bg: #101010;            /* 页面底 */
  --panel: #14161a;         /* 面板/输入框 */
  --elevated: #1c1f24;      /* 卡片/浮层/弹窗 */
  --chip: #22262c;          /* 芯片/kbd/代码底 */
  /* 边框：两级 + 一个蓝调强调边 */
  --border: #333;           /* 常规分隔/面板边/表格线 */
  --border-subtle: #2a2d33; /* 更弱的内分隔 */
  --border-accent: #3a5570; /* 蓝调:进度框/跳转按钮/引用 */
  /* 文字：亮→正文→次要→最弱 */
  --text: #e6e6e6;
  --text-body: #d0d0d0;
  --text-dim: #9aa0a6;      /* 提示/标签/说明 */
  --text-faint: #888;       /* 最弱/未判灰/轴标 */
  /* 强调与状态色（状态色仅用于三态语义） */
  --accent: #4f8cff;        /* 蓝:选中/进度/主强调 */
  --accent-soft: #cfe3ff;   /* 蓝调浅字/链接/进度框字 */
  --ok: #7fddb0;            /* 已判(Graded) */
  --warn: #ffbb77;          /* 存疑(Flagged) */
  --err: #ff7777;           /* 错误/删除/页数不符 */
  /* 圆角档位 */
  --r: 3px;                 /* 控件 */
  --r-lg: 6px;              /* 浮层/弹窗 */
}
.app-shell { display: flex; flex-direction: column; height: 100vh; background: var(--bg); }
</style>

<style scoped>
.topnav {
  display: flex; align-items: center; gap: 4px;
  padding: 0 12px; height: 44px; flex: 0 0 auto;
  background: var(--panel); border-bottom: 1px solid var(--border-subtle);
  font-family: ui-monospace, monospace; user-select: none;
}
.topnav .brand { color: var(--text-dim); font-size: 13px; margin-right: 16px; letter-spacing: 1px; }
.topnav button {
  appearance: none; background: transparent; border: none;
  color: var(--text-dim); font: inherit; font-size: 14px;
  padding: 6px 14px; height: 44px; cursor: pointer;
  border-bottom: 2px solid transparent;
}
/* 流水线序号：弱化但可见地表达"考试设置→标注→判分→导出"的顺序 */
.topnav button i {
  display: inline-block; width: 16px; height: 16px; margin-right: 6px;
  font-style: normal; font-size: 11px; line-height: 16px; text-align: center;
  border-radius: 50%; background: var(--chip); color: var(--text-faint); vertical-align: 1px;
}
.topnav button:hover { color: var(--text); }
.topnav button.on { color: var(--text); border-bottom-color: var(--accent); }
.topnav button.on i { background: var(--accent); color: #fff; }
/* 全局浅色文字：裸 <h3>/<p>/<li>/<label> 等继承此色，避免暗背景上默认黑字看不清
   （naive-ui 暗色主题只作用于其自身组件，不管原生元素）。打印时各视图 @media print 会覆盖为黑字。 */
.app-main { flex: 1 1 auto; min-height: 0; overflow: auto; color: var(--text-body); }
@media print {
  .app-shell { background: #fff !important; }
  .topnav { display: none !important; }
}
</style>
