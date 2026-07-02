<script setup lang="ts">
import { ref } from "vue";
import SetupView from "./views/SetupView.vue";
import GradeView from "./views/GradeView.vue";
const route = ref<"setup" | "grade">("setup");
</script>

<template>
  <nav>
    <button @click="route = 'setup'">考试设置</button>
    <button @click="route = 'grade'">判分</button>
  </nav>
  <SetupView v-if="route === 'setup'" />
  <GradeView v-else />
</template>
