<script setup lang="ts">
import { ref } from "vue";
import { NConfigProvider, darkTheme } from "naive-ui";
import SetupView from "./views/SetupView.vue";
import GradeView from "./views/GradeView.vue";
const route = ref<"setup" | "grade">("setup");
</script>

<template>
  <n-config-provider :theme="darkTheme" class="app-shell">
    <nav>
      <button @click="route = 'setup'">考试设置</button>
      <button @click="route = 'grade'">判分</button>
    </nav>
    <SetupView v-if="route === 'setup'" />
    <GradeView v-else />
  </n-config-provider>
</template>

<style>
.app-shell { display: block; min-height: 100vh; background: #101010; }
</style>
