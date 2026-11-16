/**
 * 线上验证墓碑清理：推一条「200 天前删除」的墓碑，看 Worker 会不会丢掉。
 * 保留期内的墓碑必须保留（离线设备要靠它知道自己删过什么），过期的才清。
 */
import { detectProxy, sendRequest } from "./proxy-fetch.mjs";

const proxy = detectProxy();
const code = process.env.TIME_SYNC_CODE;
if (!code) {
  console.error("请设置 TIME_SYNC_CODE");
  process.exit(1);
}
const base = process.env.TIME_SYNC_URL ?? "https://time-sync.zgzlcc.workers.dev";
const headers = { "X-Time-Code": code, "Content-Type": "application/json" };

const call = async (method, body) => {
  const result = await sendRequest({
    url: `${base}/api/entries`,
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    proxy,
  });
  if (result.status < 200 || result.status >= 300) throw new Error(`HTTP ${result.status}: ${result.body}`);
  return JSON.parse(result.body);
};

const now = Date.now();
const ancient = now - 200 * 86_400_000;
const recent = now - 5 * 86_400_000;

const snapshot = (await call("GET")).entries;
const before = snapshot.length;
console.log(`云端现有 ${before} 条`);

// 造两条墓碑：一条过期、一条新鲜
const probe = [
  ...snapshot,
  {
    id: "aaaaaaaa-0000-4000-8000-00000000dead",
    startMs: ancient + 1000,
    endMs: ancient + 2000,
    startDate: null,
    endDate: null,
    content: "过期墓碑探针",
    tag: "work",
    state: "completed",
    updatedAt: ancient,
    deletedAt: ancient,
  },
  {
    id: "bbbbbbbb-0000-4000-8000-00000000beef",
    startMs: recent + 1000,
    endMs: recent + 2000,
    startDate: null,
    endDate: null,
    content: "新鲜墓碑探针",
    tag: "work",
    state: "completed",
    updatedAt: recent,
    deletedAt: recent,
  },
];

const result = await call("POST", { entries: probe });
const ids = new Set(result.entries.map((entry) => entry.id));

// 推了两条，过期的被丢掉，所以净增 1 条
const expected = before + 1;
const checks = [
  ["过期墓碑被云端丢掉", !ids.has("aaaaaaaa-0000-4000-8000-00000000dead")],
  ["新鲜墓碑被保留", ids.has("bbbbbbbb-0000-4000-8000-00000000beef")],
  ["其余记录一条没少", result.entries.length === expected, `${before} → ${result.entries.length}（预期 ${expected}）`],
];

console.log("");
let failed = 0;
for (const [name, ok, detail] of checks) {
  console.log(`  ${ok ? "OK  " : "!!  "} ${name}${detail ? "  (" + detail + ")" : ""}`);
  if (!ok) failed += 1;
}

// 清理探针，别把测试数据留在用户云端。
//
// 不能靠「不带这条记录」来删——合并是后写胜出，缺了它云端那份会留着。
// 正确做法：把 deletedAt 改成很久以前（超出保留期），云端写入时会把它丢掉；
// updatedAt 用当前时间，保证这次修改能赢过云端已有的版本。
const cleaned = snapshot
  .filter((entry) => entry.id !== "bbbbbbbb-0000-4000-8000-00000000beef")
  .concat([
    {
      id: "bbbbbbbb-0000-4000-8000-00000000beef",
      startMs: ancient + 1000,
      endMs: ancient + 2000,
      startDate: null,
      endDate: null,
      content: "新鲜墓碑探针",
      tag: "work",
      state: "completed",
      updatedAt: Date.now(),
      deletedAt: ancient,
    },
  ]);
await call("POST", { entries: cleaned });
const after = (await call("GET")).entries;
const stillThere = after.some((entry) => entry.id === "bbbbbbbb-0000-4000-8000-00000000beef");
console.log(`\n探针已清理，云端 ${after.length} 条（原为 ${before}）`);
if (stillThere) {
  console.log("!! 探针仍有残留");
  failed += 1;
}

console.log(failed === 0 ? "\n墓碑清理线上验证通过" : "\n有项目未通过");
process.exit(failed === 0 ? 0 : 1);
