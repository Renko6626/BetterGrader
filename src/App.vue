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
    <nav>
      <button @click="route = 'setup'">考试设置</button>
      <button @click="route = 'label'">标注</button>
      <button @click="route = 'grade'">判分</button>
      <button @click="route = 'export'">导出</button>
    </nav>
    <SetupView v-if="route === 'setup'" />
    <LabelView v-else-if="route === 'label'" />
    <GradeView v-else-if="route === 'grade'" />
    <ExportView v-else-if="route === 'export'" />
  </n-config-provider>
</template>

<style>
.app-shell { display: block; min-height: 100vh; background: #101010; }
</style>

<style scoped>
@media print {
  .app-shell { background: #fff !important; }
  nav { display: none !important; }
}
</style>
