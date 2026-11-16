/**
 * 端到端验证 1.3.0 的升级路径：把真实的 time.sqlite3 复制到隔离目录，
 * 按 AppState::new 的逻辑走一遍（prepare_path 复制 → migrate 迁移），
 * 再逐条比对记录，确认一条不丢、一条不改。
 *
 * 全程只读原库，绝不写回。
 */
import { DatabaseSync } from "node:sqlite";
import { copyFileSync, existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const legacy = join(process.env.APPDATA, "com.time.tracker", "time.sqlite3");
const legacyHashBefore = statSync(legacy);
const legacyBytes = readFileSync(legacy);

const root = mkdtempSync(join(tmpdir(), "time-130-upgrade-"));
const data = join(root, "data");
const { mkdirSync } = await import("node:fs");
mkdirSync(data, { recursive: true });
copyFileSync(legacy, join(data, "time.sqlite3"));
console.log(`隔离目录: ${root}`);

const LEGACY = join(data, "time.sqlite3");
const TARGET = join(data, "time-v3.sqlite3");
const LOCATION = join(data, "database-location.txt");

// --- 模拟 prepare_path ---
if (existsSync(LOCATION)) throw new Error("不应存在 location 文件");
if (!existsSync(TARGET) && existsSync(LEGACY)) {
  copyFileSync(LEGACY, TARGET);
  writeFileSync(LOCATION, TARGET, "utf8");
  console.log("已复制旧库 → time-v3.sqlite3 并写下位置");
}

const original = new DatabaseSync(LEGACY, { readOnly: true });
const before = original.prepare("SELECT * FROM time_entries ORDER BY id").all();
const beforeVersion = original.prepare("PRAGMA user_version").get().user_version;
original.close();
console.log(`旧库：user_version=${beforeVersion}，${before.length} 条`);

// --- 模拟 migrate ---
// 注意：user_version 由 Rust 的 migrate_with 在每个迁移后设置，不写在 SQL 文件里，
// 所以这里也要按同样的方式补上，否则模拟出来的是「迁移跑了但版本没提升」。
const target = new DatabaseSync(TARGET);
const current = target.prepare("PRAGMA user_version").get().user_version;
console.log(`新库：user_version=${current}`);
if (current === 2) {
  const sql = readFileSync(new URL("../src-tauri/migrations/0003_sync_columns.sql", import.meta.url), "utf8");
  target.exec(sql);
  target.exec("PRAGMA user_version = 3");
  console.log("已执行 0003 迁移并置 user_version=3");
}
const afterVersion = target.prepare("PRAGMA user_version").get().user_version;
console.log(`迁移后 user_version=${afterVersion}`);

const after = target.prepare("SELECT * FROM time_entries ORDER BY id").all();

// --- 校验 ---
const fingerprint = (rows) =>
  rows
    .map((r) => [r.id, r.start_ms, r.end_ms, r.start_date, r.end_date, r.content, r.tag, r.state].join("\u0001"))
    .join("\u0002");

const checks = [
  ["记录数不变", after.length === before.length, `${before.length} → ${after.length}`],
  ["原有字段一字未改", fingerprint(after) === fingerprint(before), ""],
  ["user_version = 3", afterVersion === 3, String(afterVersion)],
  ["integrity_check = ok", target.prepare("PRAGMA integrity_check").get().integrity_check === "ok", ""],
  ["public_id 全非空唯一", new Set(after.map((r) => r.public_id)).size === after.length && after.every((r) => r.public_id.length === 36), ""],
  ["updated_at 全为正", after.every((r) => r.updated_at > 0), ""],
  ["deleted_at 全为 0", after.every((r) => r.deleted_at === 0), ""],
];

// 新写入路径：不提供 public_id 的 INSERT 应能连续成功（部分索引的效果）
let plainInsert = true;
try {
  target.exec("INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (1, 2, 'a', 'work', 'completed')");
  target.exec("INSERT INTO time_entries (start_ms, end_ms, content, tag, state) VALUES (3, 4, 'b', 'work', 'completed')");
} catch (error) {
  plainInsert = false;
  console.log("  连续两次无 public_id 插入失败:", error.message);
}
checks.push(["无 public_id 的连续插入仍可用", plainInsert, ""]);

// 模拟 1.3.0 的可见性查询
const visible = target.prepare("SELECT COUNT(*) AS n FROM time_entries WHERE deleted_at = 0").get().n;
checks.push(["未删除记录数正确", visible === before.length + (plainInsert ? 2 : 0), String(visible)]);

// 关键：原库必须一个字节都没变
const legacyBytesAfter = readFileSync(legacy);
checks.push([
  "原库字节完全未变",
  legacyBytesAfter.equals(legacyBytes) && statSync(legacy).mtimeMs === legacyHashBefore.mtimeMs,
  "",
]);

console.log("");
let failed = 0;
for (const [name, ok, detail] of checks) {
  console.log(`  ${ok ? "OK  " : "!!  "} ${name}${detail ? "  (" + detail + ")" : ""}`);
  if (!ok) failed += 1;
}

target.close();
rmSync(root, { recursive: true, force: true });
console.log(`\n${failed === 0 ? "端到端升级验证通过" : failed + " 项未通过"}，隔离目录已清理。`);
process.exit(failed === 0 ? 0 : 1);
