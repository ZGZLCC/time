<script setup lang="ts">
import { ref } from "vue";
import type { TimeEntry } from "./api";
import RecordsPage from "./features/records/RecordsPage.vue";
import WeekPage from "./features/week/WeekPage.vue";
import SettingsPage from "./features/settings/SettingsPage.vue";
import appIcon from "../src-tauri/icons/icon.svg";

const view = ref<"records" | "week" | "settings">("records");
const weekEditEntry = ref<TimeEntry | null>(null);

function editFromWeek(entry: TimeEntry) {
  weekEditEntry.value = entry;
  view.value = "records";
}

function clearWeekEditEntry() {
  weekEditEntry.value = null;
}
</script>

<template>
  <main class="app-shell">
    <header class="app-header">
      <div class="brand-mark" aria-hidden="true"><img :src="appIcon" alt="" /></div>
      <div class="brand-copy">
        <p class="eyebrow">PERSONAL TIME LOG</p>
        <h1>时间记录</h1>
      </div>
      <nav class="app-nav" aria-label="主导航">
        <button :class="['app-nav-button', { active: view === 'records' }]" type="button" :aria-current="view === 'records' ? 'page' : undefined" @click="view = 'records'">记录</button>
        <button :class="['app-nav-button', { active: view === 'week' }]" type="button" :aria-current="view === 'week' ? 'page' : undefined" @click="view = 'week'">周视图</button>
        <button :class="['app-nav-button', { active: view === 'settings' }]" type="button" :aria-current="view === 'settings' ? 'page' : undefined" @click="view = 'settings'">设置</button>
      </nav>
    </header>

    <KeepAlive>
      <RecordsPage
        v-if="view === 'records'"
        :edit-entry="weekEditEntry"
        @consume-edit="clearWeekEditEntry"
      />
      <WeekPage v-else-if="view === 'week'" @edit="editFromWeek" @open-records="view = 'records'" />
      <SettingsPage v-else />
    </KeepAlive>
  </main>
</template>
