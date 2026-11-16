/**
 * 核对原库与新库：按 public_id 比对，确认原库每条记录都能在新库找到对应。
 * 只看不写。新库多出同步拉来的记录是正常的，改动也应由同步造成——
 * 这里只关心「有没有记录凭空丢失」。
 */
import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";

const dir = join(process.env.APPDATA, "com.time.tracker");
const open = (file) => new DatabaseSync(join(dir, file), { readOnly: true });

const legacy = open("time.sqlite3");
const current = open("time-v3.sqlite3");

const legacyRows = legacy.prepare("SELECT * FROM time_entries ORDER BY id").all();
const currentRows = current.prepare("SELECT * FROM time_entries ORDER BY id").all();

console.log(`原库 ${legacyRows.length} 条，新库 ${currentRows.length} 条`);
console.log(`  新库可见 ${currentRows.filter((r) => r.deleted_at === 0).length} 条，墓碑 ${currentRows.filter((r) => r.deleted_at !== 0).length} 条`);
console.log("");

// 原库的每条按 id 找新库对应行（迁移时 id 原样保留）
const byId = new Map(currentRows.map((r) => [r.id, r]));
const missing = [];
const changed = [];

for (const old of legacyRows) {
  const now = byId.get(old.id);
  if (!now) {
    missing.push(old);
    continue;
  }
  const same =
    old.start_ms === now.start_ms &&
    old.end_ms === now.end_ms &&
    old.content === now.content &&
    old.tag === now.tag &&
    old.state === now.state;
  if (!same) changed.push({ old, now });
}

console.log(`原库记录在新库中找不到：${missing.length} 条`);
for (const row of missing) console.log(`   id=${row.id} ${String(row.content).slice(0, 24)}`);

console.log(`\n内容有变化的：${changed.length} 条`);
for (const { old, now } of changed) {
  console.log(`   id=${old.id}`);
  console.log(`     原：${JSON.stringify(old.content)} tag=${old.tag} state=${old.state} ${old.start_ms}-${old.end_ms}`);
  console.log(`     现：${JSON.stringify(now.content)} tag=${now.tag} state=${now.state} ${now.start_ms}-${now.end_ms}`);
  console.log(`     updated_at=${now.updated_at} deleted_at=${now.deleted_at}`);
}

const extra = currentRows.filter((r) => !legacyRows.some((o) => o.id === r.id));
console.log(`\n新库多出的：${extra.length} 条`);
for (const row of extra) {
  const kind = row.deleted_at !== 0 ? "墓碑" : "可见记录";
  console.log(`   id=${row.id} [${kind}] ${String(row.content).slice(0, 30)}`);
}

legacy.close();
current.close();

const lost = missing.length;
console.log(`\n${lost === 0 ? "没有任何原库记录丢失" : "!! 有 " + lost + " 条记录丢失"}`);
process.exit(lost === 0 ? 0 : 1);
