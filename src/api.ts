import { invoke } from "@tauri-apps/api/core";
import type { EntryTag } from "./tags";

export type { EntryTag } from "./tags";
export type EntryState = "running" | "draft" | "completed";
export type AppErrorCode = "validation" | "overlap" | "not_found" | "database" | "migration" | "clock";

export interface AppError {
  code: AppErrorCode;
  message: string;
  conflictingEntryId?: number;
}

export interface TimeEntry {
  id: number;
  startMs: number | null;
  endMs: number | null;
  startDate: string | null;
  endDate: string | null;
  content: string;
  tag: EntryTag | null;
  state: EntryState;
}

export interface SaveEntryInput {
  id?: number;
  startMs: number | null;
  endMs: number | null;
  startDate: string | null;
  endDate: string | null;
  content: string;
  tag: EntryTag | null;
}

export interface WeekSegment {
  entryId: number;
  startMs: number;
  endMs: number;
}

export interface WeekDay {
  date: string;
  startMs: number;
  segments: WeekSegment[];
}

export interface WeekTotals {
  workMs: number;
  leisureMs: number;
  sleepMs: number;
  otherMs: number;
  totalMs: number;
}

export interface WeekView {
  selectedDate: string;
  weekStartMs: number;
  weekEndMs: number;
  entries: TimeEntry[];
  days: WeekDay[];
  totals: WeekTotals;
}

export const greet = () => invoke<string>("greet");
export const saveEntry = (input: SaveEntryInput) =>
  invoke<TimeEntry>("save_entry", { input });
export const listEntries = () => invoke<TimeEntry[]>("list_entries");
export const listPending = () => invoke<TimeEntry[]>("list_pending");
export const deleteEntry = (id: number) => invoke<void>("delete_entry", { id });
export const startPunch = () => invoke<TimeEntry>("start_punch");
export const getWeek = (date: string) => invoke<WeekView>("get_week", { date });
export const getDayTotals = (date: string) => invoke<WeekTotals>("get_day_totals", { date });
export const getAllTotals = () => invoke<WeekTotals>("get_all_totals");
export const defaultExportDirectory = () => invoke<string>("default_export_directory");
export const saveWeekImage = (directory: string, startDate: string, endDate: string, pngBytes: number[]) =>
  invoke<string>("save_week_image", { directory, startDate, endDate, pngBytes });
export const databaseDirectory = () => invoke<string>("database_directory");
export const exportDatabase = (directory: string) => invoke<string>("export_database", { directory });
export const moveDatabase = (directory: string) => invoke<string>("move_database", { directory });

/**
 * 云端记录：字段与 Worker 的契约一致，用 UUID 与本地整数主键对应。
 * 本地主键 id 只在库内使用，跨端一律用这里的 id（即 public_id）。
 */
export interface CloudEntry {
  id: string;
  startMs: number | null;
  endMs: number | null;
  startDate: string | null;
  endDate: string | null;
  content: string;
  tag: EntryTag | null;
  state: "draft" | "completed";
  updatedAt: number;
  deletedAt: number | null;
}

export interface MergeOutcome {
  /** 从云端写回本地的条数。 */
  applied: number;
  /** 本地更新、需要推回云端的记录。 */
  pending: CloudEntry[];
}

/** 本地全部记录（含删除墓碑），转成云端格式。 */
export const cloudSnapshot = () => invoke<CloudEntry[]>("cloud_snapshot");
/** 把云端快照合并进本地，返回写回条数与待推送记录。 */
/** 同步用：用云端快照整体覆盖本地，返回写入条数。 */
export const mirrorCloud = (incoming: CloudEntry[]) => invoke<number>("mirror_cloud", { incoming });
export const mergeCloud = (incoming: CloudEntry[]) =>
  invoke<MergeOutcome>("merge_cloud", { incoming });
/** 清空全部记录（软删除，保留墓碑供同步使用），返回受影响条数。 */
export const clearEntries = () => invoke<number>("clear_entries");
/** 真正删除超过保留期的墓碑，返回删除条数。 */
export const purgeTombstones = (now: number) => invoke<number>("purge_tombstones", { now });
/** 弹出选择备份文件的对话框；用户取消时返回 null。 */
export const pickBackupFile = () => invoke<string | null>("pick_backup_file");
/** 从备份文件恢复，返回恢复的记录条数。 */
export const restoreDatabase = (path: string) => invoke<number>("restore_database", { path });
