<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import {
  clearEntries,
  defaultExportDirectory,
  exportDatabase,
  listEntries,
  listPending,
  pickBackupFile,
  restoreDatabase,
} from "../../api";
import { emitLocalDataChanged } from "../../data-events";
import { setTheme, theme, type Theme } from "../../theme";
import {
  DEFAULT_ENDPOINT,
  isConfigured,
  loadLastExportedAt,
  loadLastSyncedAt,
  loadSettings,
  markExported,
  markSynced,
  runSync,
  saveSettings,
  type SyncMode,
} from "../../sync";

const backupFolder = ref("");
const busy = ref<"backup" | "restore" | "sync" | "wipe" | null>(null);
const backupStatus = ref("");
const themeStatus = ref("");
const syncStatus = ref("");
const syncFailed = ref(false);
const backupFailed = ref(false);
const savedSettings = ref(false);
const lastSyncedAt = ref<number | null>(null);
const pendingRestore = ref<string | null>(null);
const restoreDialog = ref<HTMLDialogElement | null>(null);
const completedCount = ref(0);
const pendingCount = ref(0);
const wipeDialog = ref<HTMLDialogElement | null>(null);

const endpoint = ref(DEFAULT_ENDPOINT);
const code = ref("");
const stored = loadSettings();
endpoint.value = stored.endpoint;
code.value = stored.code;

const themes: { value: Theme; label: string }[] = [
  { value: "system", label: "跟随系统" },
  { value: "light", label: "亮色" },
  { value: "dark", label: "暗色" },
];

const configured = computed(() => isConfigured({ endpoint: endpoint.value, code: code.value }));
const isSyncing = computed(() => busy.value === "sync");
const lastSyncedText = computed(() =>
  lastSyncedAt.value === null ? "还没有成功同步过" : `上次同步 ${formatTime(lastSyncedAt.value)}`,
);
const overview = computed(
  () =>
    `已完成 ${completedCount.value} 条，待补全 ${pendingCount.value} 条。` +
    (lastExportedAt.value === null ? "还没有导出过备份" : `上次导出备份：${formatTime(lastExportedAt.value)}`),
);
const lastExportedAt = ref<number | null>(null);

function formatTime(timestamp: number): string {
  return new Date(timestamp + 8 * 3_600_000).toISOString().slice(0, 16).replace("T", " ");
}

function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error && typeof error.message === "string") {
    return error.message;
  }
  return "操作失败，请检查后重试。";
}

async function refreshOverview() {
  try {
    const [completed, pending] = await Promise.all([listEntries(), listPending()]);
    completedCount.value = completed.length;
    pendingCount.value = pending.length;
  } catch {
    completedCount.value = 0;
    pendingCount.value = 0;
  }
}

function currentSettings() {
  return { endpoint: endpoint.value.trim() || DEFAULT_ENDPOINT, code: code.value.trim() };
}

function saveSyncConfig() {
  saveSettings(currentSettings());
  savedSettings.value = true;
  window.setTimeout(() => {
    savedSettings.value = false;
  }, 2500);
}

/** 与网页版一致：立即同步 / 上传本地记录 / 从云端恢复。 */
/**
 * 覆盖类的同步要先确认：这两个方向都会整份替换另一端的数据，
 * 和「立即同步」的逐条合并不是一回事。
 */
const pendingSync = ref<SyncMode | null>(null);
const syncDialog = ref<HTMLDialogElement | null>(null);

const syncDialogCopy = computed(() => {
  if (pendingSync.value === "push") {
    return {
      title: "用本地覆盖云端",
      body: "云端的全部记录会被本机内容替换，云端多出来的记录将被删除。其他设备下次同步时会跟着删除。确定继续吗？",
      confirm: "确定覆盖",
    };
  }
  return {
    title: "用云端覆盖本地",
    body: "本机的全部记录会被云端内容替换，本机多出来的记录将被删除。确定继续吗？",
    confirm: "确定覆盖",
  };
});

async function requestSync(mode: SyncMode) {
  if (busy.value || !configured.value) return;
  if (mode === "both") {
    await syncNow("both");
    return;
  }
  pendingSync.value = mode;
  await nextTick();
  syncDialog.value?.showModal();
}

function cancelSync() {
  if (busy.value === "sync") return;
  pendingSync.value = null;
  syncDialog.value?.close();
}

async function confirmSync() {
  const mode = pendingSync.value;
  syncDialog.value?.close();
  pendingSync.value = null;
  if (!mode) return;
  await syncNow(mode);
}

async function syncNow(mode: SyncMode) {
  if (busy.value || !configured.value) return;
  busy.value = "sync";
  syncStatus.value = "";
  try {
    saveSettings(currentSettings());
    const result = await runSync(currentSettings(), mode);
    const now = Date.now();
    markSynced(now);
    lastSyncedAt.value = now;
    await refreshOverview();
    // 记录页被 KeepAlive 缓存，不通知就会一直停在同步前的旧列表上
    emitLocalDataChanged();
    syncFailed.value = false;
    const action = mode === "push" ? "已用本地覆盖云端" : mode === "pull" ? "已用云端覆盖本地" : "已同步";
    syncStatus.value =
      `${action}：云端写回 ${result.applied} 条，推送 ${result.pushed} 条，云端共 ${result.total} 条。`;
  } catch (error) {
    syncFailed.value = true;
    syncStatus.value = errorMessage(error);
  } finally {
    busy.value = null;
  }
}

async function requestWipe() {
  if (busy.value) return;
  await nextTick();
  wipeDialog.value?.showModal();
}

async function confirmWipe() {
  wipeDialog.value?.close();
  if (busy.value) return;
  busy.value = "wipe";
  try {
    const affected = await clearEntries();
    backupFailed.value = false;
    backupStatus.value = `已清空 ${affected} 条记录（保留删除标记，避免其他设备同步时又拉回来）。`;
    await refreshOverview();
    emitLocalDataChanged();
  } catch (error) {
    backupFailed.value = true;
    backupStatus.value = errorMessage(error);
  } finally {
    busy.value = null;
  }
}

onMounted(async () => {
  lastSyncedAt.value = loadLastSyncedAt();
  lastExportedAt.value = loadLastExportedAt();
  await refreshOverview();
  try {
    backupFolder.value = await defaultExportDirectory();
  } catch (error) {
    backupFailed.value = true;
    backupStatus.value = errorMessage(error);
  }
});

/** 挑一个备份文件，弹确认框——和网页版一样先确认再覆盖。 */
async function chooseRestoreFile() {
  if (busy.value) return;
  try {
    const selected = await pickBackupFile();
    if (!selected) return;
    pendingRestore.value = selected;
    await nextTick();
    restoreDialog.value?.showModal();
  } catch (error) {
    backupFailed.value = true;
    backupStatus.value = errorMessage(error);
  }
}

/** 去掉 Windows 扩展长度前缀，提示里显示成用户认得的普通路径。 */
function friendlyPath(path: string): string {
  return path.replace(/^\\\\\?\\/, "");
}

function cancelRestore() {
  pendingRestore.value = null;
  restoreDialog.value?.close();
}

async function applyRestore() {
  const path = pendingRestore.value;
  if (!path || busy.value) return;
  busy.value = "restore";
  try {
    const restored = await restoreDatabase(path);
    backupFailed.value = false;
    backupStatus.value = `已从 ${fileName(path)} 恢复 ${restored} 条记录。`;
    cancelRestore();
    await refreshOverview();
    emitLocalDataChanged();
  } catch (error) {
    backupFailed.value = true;
    backupStatus.value = errorMessage(error);
    restoreDialog.value?.close();
    pendingRestore.value = null;
  } finally {
    busy.value = null;
  }
}

/** 只取路径里的文件名，用于提示文案。 */
function fileName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

async function saveBackup() {
  if (!backupFolder.value.trim() || busy.value) return;
  busy.value = "backup";
  backupStatus.value = "";
  try {
    const path = await exportDatabase(backupFolder.value.trim());
    const now = Date.now();
    markExported(now);
    lastExportedAt.value = now;
    backupFailed.value = false;
    backupStatus.value = `已导出：${friendlyPath(path)}`;
  } catch (error) {
    backupFailed.value = true;
    backupStatus.value = errorMessage(error);
  } finally {
    busy.value = null;
  }
}

function chooseTheme(value: Theme) {
  try {
    setTheme(value);
    themeStatus.value = "";
  } catch {
    themeStatus.value = "无法保存主题设置，请检查应用数据权限后重试。";
  }
}
</script>

<template>
  <div class="settings-page">
    <div class="settings-heading">
      <p class="eyebrow">应用偏好</p>
      <h2>设置</h2>
    </div>

    <section class="panel settings-card" aria-labelledby="cloud-title">
      <h3 id="cloud-title">云端同步</h3>
      <p>
        填入管理员给你的同步码后，桌面记录会与网页版同步到同一份数据，换设备填入同一个码即可拉回全部记录。
        一个同步码对应一份独立数据，码只保存在本机。同步走系统代理，无需额外配置。
      </p>
      <div class="field-stack">
        <label class="field">
          <span>同步码</span>
          <input v-model="code" type="text" placeholder="time_ 开头的一串字符" spellcheck="false" autocomplete="off" />
        </label>
      </div>
      <div class="backup-actions">
        <button class="button button-secondary" type="button" :disabled="isSyncing" @click="saveSyncConfig">{{ savedSettings ? "已保存" : "保存配置" }}</button>
        <button class="button button-secondary" type="button" :disabled="isSyncing || !configured" @click="requestSync('both')">立即同步</button>
        <button class="button button-secondary" type="button" :disabled="isSyncing || !configured" @click="requestSync('push')">上传本地记录</button>
        <button class="button button-secondary" type="button" :disabled="isSyncing || !configured" @click="requestSync('pull')">从云端恢复</button>
      </div>
      <p class="folder-message" role="status">{{ lastSyncedText }}</p>
      <p v-if="syncStatus" :class="['folder-message', { 'status-error': syncFailed }]" :role="syncFailed ? 'alert' : 'status'">{{ syncStatus }}</p>
    </section>

    <section class="panel settings-card" aria-labelledby="backup-title">
      <h3 id="backup-title">备份与恢复</h3>
      <p>导出会把全部记录保存成一个 SQLite 备份文件（同名文件不会被覆盖）；恢复会用备份内容整体替换当前记录，替换后无法撤销，建议先导出一份。换机器或重装系统前同样建议先导出。</p>
      <div class="backup-actions">
        <button class="button button-secondary" type="button" :disabled="!!busy || !backupFolder.trim()" @click="saveBackup">{{ busy === "backup" ? "正在导出…" : "导出备份" }}</button>
        <button class="button button-secondary" type="button" :disabled="!!busy" @click="chooseRestoreFile">{{ busy === "restore" ? "正在恢复…" : "从文件恢复" }}</button>
        <button class="button button-secondary" type="button" :disabled="!!busy" @click="requestWipe">{{ busy === "wipe" ? "正在清空…" : "清空记录" }}</button>
      </div>
      <p v-if="backupStatus" :class="['folder-message', { 'status-error': backupFailed }]" :role="backupFailed ? 'alert' : 'status'">{{ backupStatus }}</p>
    </section>

    <section class="panel settings-card" aria-labelledby="storage-title">
      <h3 id="storage-title">本地数据</h3>
      <p>记录保存在本机的 SQLite 数据库里；未配置云端同步或同步不通时，它就是唯一一份。</p>
      <p class="folder-message" role="status">{{ overview }}</p>
    </section>

    <section class="panel settings-card" aria-labelledby="theme-title">
      <h3 id="theme-title">修改主题</h3>
      <p>跟随系统会在系统外观变化时自动切换。</p>
      <div class="theme-options" role="group" aria-label="应用主题">
        <button v-for="option in themes" :key="option.value" class="button theme-option" :class="{ selected: theme === option.value }" type="button" :aria-pressed="theme === option.value" @click="chooseTheme(option.value)">{{ option.label }}</button>
      </div>
      <p v-if="themeStatus" class="folder-message status-error" role="alert">{{ themeStatus }}</p>
    </section>

    <dialog ref="syncDialog" class="delete-dialog" aria-labelledby="sync-title">
      <h2 id="sync-title">{{ syncDialogCopy.title }}</h2>
      <p>{{ syncDialogCopy.body }}</p>
      <div class="delete-actions">
        <button class="button button-danger-quiet" type="button" :disabled="busy === 'sync'" @click="confirmSync">
          {{ busy === "sync" ? "正在同步…" : syncDialogCopy.confirm }}
        </button>
        <button class="button button-quiet" type="button" :disabled="busy === 'sync'" @click="cancelSync">取消</button>
      </div>
    </dialog>

    <dialog ref="restoreDialog" class="delete-dialog" aria-labelledby="restore-title">
      <h2 id="restore-title">恢复备份</h2>
      <p>将用 {{ pendingRestore ? fileName(pendingRestore) : "" }} 替换当前全部记录，现有记录会被覆盖。确定继续吗？</p>
      <div class="delete-actions">
        <button class="button button-primary" type="button" :disabled="busy === 'restore'" @click="applyRestore">确定恢复</button>
        <button class="button button-quiet" type="button" :disabled="busy === 'restore'" @click="cancelRestore">取消</button>
      </div>
    </dialog>

    <dialog ref="wipeDialog" class="delete-dialog" aria-labelledby="wipe-title">
      <h2 id="wipe-title">清空记录</h2>
      <p>确定清空全部记录吗？此操作无法撤销，建议先导出备份。</p>
      <div class="delete-actions">
        <button class="button button-danger-quiet" type="button" :disabled="busy === 'wipe'" @click="confirmWipe">
          {{ busy === "wipe" ? "正在清空…" : "清空记录" }}
        </button>
        <button class="button button-quiet" type="button" :disabled="busy === 'wipe'" @click="wipeDialog?.close()">取消</button>
      </div>
    </dialog>
  </div>
</template>
