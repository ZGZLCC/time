<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type { WeekView } from "../../api";
import { defaultExportDirectory, saveWeekImage } from "../../api";
import { renderWeekImage } from "./week-image";

const props = defineProps<{ week: WeekView }>();
const directory = ref("");
const exporting = ref(false);
const message = ref("");
const hasError = ref(false);

onMounted(async () => {
  try {
    const downloads = await defaultExportDirectory();
    if (!directory.value) directory.value = downloads;
  } catch (error) {
    showError(error);
  }
});

function showError(error: unknown) {
  hasError.value = true;
  message.value = typeof error === "object" && error !== null && "message" in error && typeof error.message === "string"
    ? error.message : "操作失败，请检查导出位置后重试。";
}

async function chooseFolder() {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      directory.value = selected;
      message.value = "";
    }
  } catch (error) {
    showError(error);
  }
}

async function exportImage() {
  const target = directory.value.trim();
  if (!target || exporting.value) return;
  exporting.value = true;
  message.value = "";
  try {
    const week = props.week;
    const pngBytes = await renderWeekImage(week);
    const path = await saveWeekImage(target, week.days[0].date, week.days[6].date, pngBytes);
    hasError.value = false;
    message.value = `已导出：${path}`;
  } catch (error) {
    showError(error);
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <div class="week-export">
    <h3>导出周记录图片</h3>
    <div class="folder-controls">
      <label class="sr-only" for="week-export-directory">导出文件夹地址</label>
      <input id="week-export-directory" v-model="directory" type="text" placeholder="输入导出文件夹地址" spellcheck="false" />
      <button class="button button-secondary" type="button" :disabled="exporting" @click="chooseFolder">选择文件夹</button>
      <button class="button button-primary" type="button" :disabled="!directory.trim() || exporting" @click="exportImage">{{ exporting ? "正在导出…" : "导出图片" }}</button>
    </div>
    <p v-if="message" :class="['folder-message', { 'status-error': hasError }]" :role="hasError ? 'alert' : 'status'">{{ message }}</p>
  </div>
</template>
