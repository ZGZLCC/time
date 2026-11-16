/**
 * 线上验证「覆盖」语义：用真实 Worker 与同步码，确认
 *   ① 本地删掉记录后上传，云端也会删掉（而不是被云端旧记录拉回来）
 *   ② 云端多出来的记录在「上传本地记录」后消失
 *   ③ 反过来「从云端恢复」能把云端内容整体落下来
 *
 * 全程用一个独立的临时同步码，不碰你自己的数据。
 */
import { detectProxy, sendRequest } from "./proxy-fetch.mjs";

const CODE = process.env.TIME_SYNC_CODE;
if (!CODE) {
  console.error("请设置 TIME_SYNC_CODE（用一个临时码，本脚本会整份覆盖它）");
  process.exit(1);
}
const BASE = process.env.TIME_SYNC_URL ?? "https://time-sync.zgzlcc.workers.dev";
const PROXY = detectProxy();
const headers = { "X-Time-Code": CODE, "Content-Type": "application/json" };

const cloud = async (method, body, suffix = "") => {
  const result = await sendRequest({
    url: `${BASE}/api/entries${suffix}`,
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

/** 「用本地覆盖云端」走的就是带 replace=1 的 POST。 */
const replace = (entries) => cloud("POST", { entries }, "?replace=1");

/** 造一条云端格式的记录。 */
const entry = (id, content, updatedAt, deletedAt = null) => ({
  id,
  startMs: updatedAt,
  endMs: updatedAt + 60_000,
  startDate: null,
  endDate: null,
  content,
  tag: "work",
  state: "completed",
  updatedAt,
  deletedAt,
});

const checks = [];
const check = (name, ok, detail = "") => {
  checks.push({ name, ok });
  console.log(`  ${ok ? "OK  " : "!!  "} ${name}${detail ? `  (${detail})` : ""}`);
};

const now = Date.now();
const A = "aaaaaaaa-1111-4111-8111-aaaaaaaaaaaa";
const B = "bbbbbbbb-2222-4222-8222-bbbbbbbbbbbb";
const C = "cccccccc-3333-4333-8333-cccccccccccc";

// ① 本地两条 → 覆盖上传，云端就是两条
const first = await replace([entry(A, "甲", now), entry(B, "乙", now + 1000)]);
check("覆盖上传两条后云端有两条", first.entries.length === 2, `实际 ${first.entries.length}`);

// ② 云端再塞一条本机没有的记录，模拟「另一端写过」
await replace([entry(A, "甲", now), entry(B, "乙", now + 1000), entry(C, "丙（本机没有）", now + 2000)]);
const before = await cloud("GET");
check("先把云端做成三条", before.entries.length === 3, `实际 ${before.entries.length}`);

// ③ 本机只推两条（用本地的两条覆盖云端）——丙应当从云端消失
const mirrored = await replace([entry(A, "甲", now), entry(B, "乙", now + 1000)]);
const afterOverride = await cloud("GET");
const visible = afterOverride.entries.filter((item) => item.deletedAt === null);
check(
  "用本地两条覆盖后，云端可见记录只剩两条",
  visible.length === 2,
  `可见 ${visible.length}：${visible.map((item) => item.content).join("、")}`,
);
check("丙没有重新出现", !visible.some((item) => item.id === C), visible.some((item) => item.id === C) ? "丙还在" : "丙已不在");
check(
  "覆盖的返回内容里也不含丙",
  !(mirrored.entries ?? []).some((item) => item.id === C && item.deletedAt === null),
  "返回内容就是落盘结果，不该把云端多出来的带回来",
);

// ④ 本地都删掉（推两条墓碑）→ 云端可见记录应为 0
const tombstoned = await replace([entry(A, "甲", now + 5000, now + 5000), entry(B, "乙", now + 5000, now + 5000)]);
const visibleAfterDelete = tombstoned.entries.filter((item) => item.deletedAt === null);
check("推墓碑后云端可见记录为 0", visibleAfterDelete.length === 0, `实际 ${visibleAfterDelete.length}`);

// ⑤ 不带 replace 时仍是合并：云端多出来的记录要留着（「立即同步」依赖这个）
await replace([entry(C, "丙", now + 6000)]);
const merged = await cloud("POST", { entries: [entry(A, "甲", now + 7000), entry(B, "乙", now + 7000)] });
const visibleMerged = merged.entries.filter((item) => item.deletedAt === null);
check(
  "不带 replace 时仍是逐条合并（保留云端独有的丙）",
  visibleMerged.length === 3,
  `可见 ${visibleMerged.length}：${visibleMerged.map((item) => item.content).join("、")}`,
);

// 收尾：把这个临时码的数据清干净
await replace([
  entry(A, "甲", now + 9000, now + 9000),
  entry(B, "乙", now + 9000, now + 9000),
  entry(C, "丙", now + 9000, now + 9000),
]);
const final = await cloud("GET");
console.log(`\n临时码数据已清空（剩余 ${final.entries.length} 条，均为墓碑）`);

const failed = checks.filter((item) => !item.ok).length;
console.log(failed === 0 ? `覆盖语义线上验证：${checks.length}/${checks.length} 项通过` : `有 ${failed} 项未通过`);
process.exit(failed === 0 ? 0 : 1);
