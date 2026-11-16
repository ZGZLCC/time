/**
 * 用真实的 Worker 与同步码，在数据库副本上跑一遍桌面版同步流程。
 * 验证的是「桌面 Rust 侧的合并逻辑 + Worker 契约」能否对接成功。
 *
 * 只在隔离目录里操作，绝不碰真实数据库。
 */
import { DatabaseSync } from "node:sqlite";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { detectProxy, sendRequest } from "./proxy-fetch.mjs";

const CODE = process.env.TIME_SYNC_CODE;
if (!CODE) {
  console.error("请设置 TIME_SYNC_CODE");
  process.exit(1);
}
const BASE = process.env.TIME_SYNC_URL ?? "https://time-sync.zgzlcc.workers.dev";
const PROXY = detectProxy();
console.log(`服务地址 ${BASE}，代理 ${PROXY || "未检测到"}`);
const LEGACY = join(process.env.APPDATA, "com.time.tracker", "time.sqlite3");

// ---------- 准备一个隔离的新库 ----------
const root = mkdtempSync(join(tmpdir(), "time-sync-live-"));
const dbPath = join(root, "time-v3.sqlite3");
copyFileSync(LEGACY, dbPath);
const db = new DatabaseSync(dbPath);
if (db.prepare("PRAGMA user_version").get().user_version === 2) {
  db.exec(readFileSync(new URL("../src-tauri/migrations/0003_sync_columns.sql", import.meta.url), "utf8"));
  db.exec("PRAGMA user_version = 3");
}
const localCount = () => db.prepare("SELECT COUNT(*) AS n FROM time_entries WHERE deleted_at = 0").get().n;
console.log(`隔离库就绪：${localCount()} 条可见记录（未删除）`);

// ---------- 复刻 Rust 的 cloud_snapshot ----------
const snapshot = () =>
  db
    .prepare(
      "SELECT public_id, start_ms, end_ms, start_date, end_date, content, tag, state, updated_at, deleted_at FROM time_entries",
    )
    .all()
    .map((row) => ({
      id: row.public_id,
      startMs: row.start_ms,
      endMs: row.end_ms,
      startDate: row.start_date,
      endDate: row.end_date,
      content: row.content,
      tag: row.tag,
      // 云端的 running 对应桌面端的 draft
      state: row.state === "completed" ? "completed" : "draft",
      updatedAt: row.updated_at,
      deletedAt: row.deleted_at === 0 ? null : row.deleted_at,
    }));

// ---------- 复刻 Rust 的 merge_cloud_entries ----------
function mergeCloud(incoming) {
  let applied = 0;
  for (const entry of incoming) {
    if (!entry.id || !entry.id.trim()) continue;
    const state = entry.state === "completed" ? "completed" : "draft";
    const deletedAt = entry.deletedAt ?? 0;
    if (state === "completed" && (!entry.startMs || !entry.endMs || !entry.content?.trim() || !entry.tag)) continue;
    const existing = db
      .prepare("SELECT id, updated_at FROM time_entries WHERE public_id = ?")
      .get(entry.id);
    if (existing) {
      if (entry.updatedAt > existing.updated_at) {
        db.prepare(
          "UPDATE time_entries SET start_ms=?, end_ms=?, start_date=?, end_date=?, content=?, tag=?, state=?, updated_at=?, deleted_at=? WHERE id=?",
        ).run(
          entry.startMs, entry.endMs, entry.startDate, entry.endDate, entry.content,
          entry.tag, state, entry.updatedAt, deletedAt, existing.id,
        );
        applied += 1;
      }
    } else {
      db.prepare(
        "INSERT INTO time_entries (public_id, start_ms, end_ms, start_date, end_date, content, tag, state, updated_at, deleted_at) VALUES (?,?,?,?,?,?,?,?,?,?)",
      ).run(
        entry.id, entry.startMs, entry.endMs, entry.startDate, entry.endDate,
        entry.content, entry.tag, state, entry.updatedAt, deletedAt,
      );
      applied += 1;
    }
  }
  // 本地更新、需要推回云端的
  const incomingUpdated = new Map(incoming.map((e) => [e.id, e.updatedAt]));
  const pending = snapshot().filter((local) => {
    const remote = incomingUpdated.get(local.id);
    return remote === undefined || local.updatedAt > remote;
  });
  return { applied, pending };
}

const headers = { "X-Time-Code": CODE, "Content-Type": "application/json" };
const cloud = async (method, body) => {
  const result = await sendRequest({
    url: `${BASE}/api/entries`,
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    proxy: PROXY,
  });
  if (result.status < 200 || result.status >= 300) {
    throw new Error(`HTTP ${result.status}: ${result.body}`);
  }
  return JSON.parse(result.body);
};

const checks = [];
const check = (name, ok, detail = "") => {
  checks.push([name, ok, detail]);
  console.log(`  ${ok ? "OK  " : "!!  "} ${name}${detail ? "  (" + detail + ")" : ""}`);
};

console.log("\n=== 第一轮：推送 → 合并 → 回读 ===");
const remote1 = await cloud("GET");
console.log(`  云端现有 ${remote1.entries.length} 条`);
const outcome1 = mergeCloud(remote1.entries);
console.log(`  合并写回本地 ${outcome1.applied} 条，待推送 ${outcome1.pending.length} 条`);

const pushed = await cloud("POST", { entries: snapshot() });
console.log(`  推送后台云端共 ${pushed.entries.length} 条`);
const outcome2 = mergeCloud(pushed.entries ?? []);

check("推送后云端条数不少于本地条数", pushed.entries.length >= localCount(), `${pushed.entries.length} vs ${localCount()}`);
check("回读合并没有产生新的待推送", outcome2.pending.length === 0, `${outcome2.pending.length} 条`);

console.log("\n=== 第二轮：模拟另一台设备，从云端拉回 ===");
const freshPath = join(root, "fresh.sqlite3");
const fresh = new DatabaseSync(freshPath);
fresh.exec(readFileSync(new URL("../src-tauri/migrations/0001_initial.sql", import.meta.url), "utf8"));
fresh.exec(readFileSync(new URL("../src-tauri/migrations/0002_partial_entries.sql", import.meta.url), "utf8"));
fresh.exec(readFileSync(new URL("../src-tauri/migrations/0003_sync_columns.sql", import.meta.url), "utf8"));
fresh.exec("PRAGMA user_version = 3");
const remote2 = await cloud("GET");
let inserted = 0;
for (const entry of remote2.entries) {
  if (!entry.id) continue;
  fresh
    .prepare(
      "INSERT OR IGNORE INTO time_entries (public_id, start_ms, end_ms, start_date, end_date, content, tag, state, updated_at, deleted_at) VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .run(
      entry.id, entry.startMs, entry.endMs, entry.startDate, entry.endDate,
      entry.content, entry.tag, entry.state === "completed" ? "completed" : "draft",
      entry.updatedAt, entry.deletedAt ?? 0,
    );
  inserted += 1;
}
const freshVisible = fresh.prepare("SELECT COUNT(*) AS n FROM time_entries WHERE deleted_at = 0").get().n;
check("全新设备能从云端拉回全部记录", freshVisible === localCount(), `${freshVisible} vs 本地 ${localCount()}`);

console.log("\n=== 第三轮：改一条并发起同步，确认云端跟着变 ===");
const target = snapshot().find((entry) => entry.content && entry.content.trim() && entry.deletedAt === null);
if (target) {
  const newContent = `${target.content}（同步验证）`;
  // 先取一次云端快照作为「当前云端状态」，再改本地——顺序反了就会拿旧快照去比
  const beforeEdit = await cloud("GET");
  db.prepare("UPDATE time_entries SET content = ?, updated_at = ? WHERE public_id = ?").run(
    newContent,
    Date.now(),
    target.id,
  );
  const outcome3 = mergeCloud(beforeEdit.entries);
  check(
    "本地改动被识别为待推送",
    outcome3.pending.some((e) => e.id === target.id),
    `待推送 ${outcome3.pending.length} 条`,
  );
  const after = await cloud("POST", { entries: snapshot() });
  const found = (after.entries ?? []).find((e) => e.id === target.id);
  check("推送后云端内容已更新", !!found && found.content === newContent, found ? found.content : "没找到");
} else {
  console.log("  跳过：本地没有可改的已完成记录");
}

db.close();
fresh.close();
rmSync(root, { recursive: true, force: true });

const failed = checks.filter(([, ok]) => !ok).length;
console.log(`\n${failed === 0 ? "桌面同步端到端联调通过" : failed + " 项未通过"}，隔离目录已清理。`);
process.exit(failed === 0 ? 0 : 1);
