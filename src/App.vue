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
      <button :class="{ on: route === 'setup' }" @click="route = 'setup'">考试设置</button>
      <button :class="{ on: route === 'label' }" @click="route = 'label'">标注</button>
      <button :class="{ on: route === 'grade' }" @click="route = 'grade'">判分</button>
      <button :class="{ on: route === 'export' }" @click="route = 'export'">导出</button>
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
.app-shell { display: flex; flex-direction: column; height: 100vh; background: #101010; }
</style>

<style scoped>
.topnav {
  display: flex; align-items: center; gap: 4px;
  padding: 0 12px; height: 44px; flex: 0 0 auto;
  background: #16181c; border-bottom: 1px solid #2a2d33;
  font-family: ui-monospace, monospace; user-select: none;
}
.topnav .brand { color: #6b7280; font-size: 13px; margin-right: 16px; letter-spacing: 1px; }
.topnav button {
  appearance: none; background: transparent; border: none;
  color: #9aa0a6; font: inherit; font-size: 14px;
  padding: 6px 14px; height: 44px; cursor: pointer;
  border-bottom: 2px solid transparent;
}
.topnav button:hover { color: #e6e6e6; }
.topnav button.on { color: #fff; border-bottom-color: #4f8cff; }
.app-main { flex: 1 1 auto; min-height: 0; overflow: auto; }
@media print {
  .app-shell { background: #fff !important; }
  .topnav { display: none !important; }
}
</style>
