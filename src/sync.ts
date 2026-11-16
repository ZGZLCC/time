/**
 * 云端同步：与网页版对接同一个 Worker 和同一个同步码。
 *
 * 数据方向由 Rust 侧决定（merge_cloud 既写回本地、又返回需要推上去的更晚版本），
 * 这里只负责三件事：读写本机配置、发 HTTP 请求、把结果整理成界面要的状态。
 */
import {
  cloudSnapshot,
  mergeCloud,
  mirrorCloud,
  purgeTombstones as purgeTombstonesCommand,
  type CloudEntry,
} from "./api";

export type SyncState = "idle" | "syncing" | "synced" | "offline" | "error";

export interface SyncSettings {
  /** 同步服务地址；构建时给了默认值，一般不用改。 */
  endpoint: string;
  /** 同步码：由管理员创建，一个码对应一份独立数据。 */
  code: string;
}

const SETTINGS_KEY = "time-sync";
const LAST_SYNCED_KEY = "time-synced-at";
const LAST_EXPORT_KEY = "time-last-export";

/** 默认服务地址，与网页版一致。 */
export const DEFAULT_ENDPOINT = "https://time-sync.zgzlcc.workers.dev";

export function loadSettings(): SyncSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return { endpoint: DEFAULT_ENDPOINT, code: "" };
    const value = JSON.parse(raw) as { endpoint?: unknown; code?: unknown };
    return {
      endpoint: String(value.endpoint ?? "").trim() || DEFAULT_ENDPOINT,
      code: String(value.code ?? "").trim(),
    };
  } catch {
    return { endpoint: DEFAULT_ENDPOINT, code: "" };
  }
}

export function saveSettings(settings: SyncSettings): void {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
}

export function isConfigured(settings: SyncSettings): boolean {
  return settings.endpoint.trim() !== "" && settings.code.trim() !== "";
}

function readNumber(key: string): number | null {
  try {
    const raw = localStorage.getItem(key);
    const value = raw === null ? Number.NaN : Number(raw);
    return Number.isFinite(value) ? value : null;
  } catch {
    return null;
  }
}

function writeNumber(key: string, value: number): void {
  try {
    localStorage.setItem(key, String(value));
  } catch {
    // 本地存储不可用时只是记不住时间，不影响同步本身
  }
}

export function loadLastSyncedAt(): number | null {
  return readNumber(LAST_SYNCED_KEY);
}

export function loadLastExportedAt(): number | null {
  return readNumber(LAST_EXPORT_KEY);
}

export function markSynced(now: number): void {
  writeNumber(LAST_SYNCED_KEY, now);
}

export function markExported(now: number): void {
  writeNumber(LAST_EXPORT_KEY, now);
}

export interface SyncResult {
  /** 从云端写回本地的条数。 */
  applied: number;
  /** 推送到云端的条数。 */
  pushed: number;
  /** 云端合并后的总条数。 */
  total: number;
}

/**
 * 同步方向，与设置页的三个按钮对应：
 *   push — 用本地覆盖云端（云端变成本地的镜像）
 *   pull — 用云端覆盖本地（本地变成云端的镜像）
 *   both — 逐条「后写胜出」的双向合并（「立即同步」）
 */
export type SyncMode = "push" | "pull" | "both";

/**
 * 一次同步。
 *
 * push 与 pull 是**覆盖**而不是合并：这两个方向都要求单向权威。
 * 旧实现在 push 之后还会把云端快照合并回本地，等于顺手拉了一次，
 * 「用本地覆盖云端」会被云端已有的旧记录污染；改成覆盖后这个副作用没有了。
 *
 * 覆盖的做法：先把本地或云端定成权威，再全量推上去，
 * 于是云端与本机完全一致；被删掉的记录靠墓碑传递到其他设备。
 */
export async function runSync(settings: SyncSettings, mode: SyncMode = "both"): Promise<SyncResult> {
  const base = settings.endpoint.trim().replace(/\/+$/, "");
  const headers = { "X-Time-Code": settings.code.trim(), "Content-Type": "application/json" };

  if (mode === "pull") {
    // 用云端覆盖本地：快照里没有的记录会被打上删除标记
    const remote = await request(base, headers, "GET");
    const applied = await mirrorCloud(remote.entries ?? []);
    await purgeExpiredTombstones();
    return { applied, pushed: 0, total: remote.entries?.length ?? 0 };
  }

  if (mode === "push") {
    // 用本地覆盖云端：带 replace=1 上传，云端直接以本地全量为准。
    //
    // 这里**不能**把返回结果再镜像回本地：Worker 在没有 replace 时是按条合并，
    // 云端独有的记录会被合并进返回结果，镜像回来就等于顺手拉了一次
    // ——「上传」就变成了「先传后拉」，正是之前那个 bug。
    const snapshot = await cloudSnapshot();
    const merged = await request(base, headers, "POST", { entries: snapshot }, "?replace=1");
    await purgeExpiredTombstones();
    return { applied: 0, pushed: snapshot.length, total: merged.entries?.length ?? 0 };
  }

  // both：双向合并
  let remote = await request(base, headers, "GET");
  let total = remote.entries?.length ?? 0;

  const snapshot = await cloudSnapshot();
  const merged = await request(base, headers, "POST", { entries: snapshot });
  total = merged.entries?.length ?? total;
  let applied = (await mergeCloud(merged.entries ?? [])).applied;

  // 再拉一次，确保没有因为推送时机错过的更新
  remote = await request(base, headers, "GET");
  applied += (await mergeCloud(remote.entries ?? [])).applied;
  total = remote.entries?.length ?? total;

  // 同步完成后清掉过期墓碑：必须在同步之后做，先清会导致本地没有墓碑可推，
  // 而云端若也已清，别的设备的旧副本就能把记录推回来。
  await purgeExpiredTombstones();

  return { applied, pushed: snapshot.length, total };
}

/** 清掉过期墓碑。失败只影响占用空间，所以吞掉错误。 */
async function purgeExpiredTombstones(): Promise<void> {
  try {
    await purgeTombstonesCommand(Date.now());
  } catch {
    // 下次同步再试
  }
}

interface EntriesPayload {
  revision?: number;
  entries?: CloudEntry[];
}

async function request(
  base: string,
  headers: Record<string, string>,
  method: "GET" | "POST",
  body?: unknown,
  suffix = "",
): Promise<EntriesPayload> {
  let response: Response;
  try {
    response = await fetch(`${base}/api/entries${suffix}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new SyncError(
      `连不上 ${base}。国内直连 *.workers.dev 会被 DNS 污染，需要在系统里开启代理；也可能是地址填错。`,
    );
  }
  if (response.status === 400) {
    throw new SyncError("同步码格式无效，请确认完整复制了管理员给你的码。");
  }
  if (response.status === 401) {
    throw new SyncError("这个同步码不存在或已被停用，请向管理员索取新的码。");
  }
  if (!response.ok) {
    throw new SyncError(`云端返回 ${response.status}，同步失败。`);
  }
  try {
    return (await response.json()) as EntriesPayload;
  } catch {
    throw new SyncError("云端返回的内容无法解析，请确认地址指向 Time 同步服务。");
  }
}

/** 带中文提示的同步错误。 */
export class SyncError extends Error {}
