/** 看一眼某个同步码下云端存了什么，用于排查同步问题。 */
import { detectProxy, sendRequest } from "./proxy-fetch.mjs";

const proxy = detectProxy();
const code = process.env.TIME_SYNC_CODE;
if (!code) {
  console.error("请设置 TIME_SYNC_CODE");
  process.exit(1);
}
const base = process.env.TIME_SYNC_URL ?? "https://time-sync.zgzlcc.workers.dev";

const result = await sendRequest({
  url: `${base}/api/entries`,
  method: "GET",
  headers: { "X-Time-Code": code },
  proxy,
});
if (result.status !== 200) {
  console.error(`HTTP ${result.status}: ${result.body}`);
  process.exit(1);
}
const data = JSON.parse(result.body);
const entries = data.entries ?? [];
console.log(`云端共 ${entries.length} 条（revision ${data.revision}）`);
for (const entry of entries) {
  const when = entry.startMs
    ? new Date(entry.startMs + 8 * 3_600_000).toISOString().slice(0, 16).replace("T", " ")
    : "（无开始时间）";
  const deleted = entry.deletedAt ? " [已删除]" : "";
  console.log(`  ${String(entry.id).slice(0, 8)}…  ${when}  ${entry.tag ?? "-"}  ${String(entry.content).slice(0, 40)}${deleted}`);
}
