import assert from "node:assert/strict";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import vuePlugin from "@vitejs/plugin-vue";
import { h } from "vue";
import { renderToString } from "vue/server-renderer";

// SFC 模板需要带编译器的完整 Vue 构建，默认运行时构建在 Node 下无法编译模板。
const vueEntry = fileURLToPath(new URL("../node_modules/vue/dist/vue.esm-bundler.js", import.meta.url));

function entry(id, startMs, endMs, content, tag = "work") {
  return { id, startMs, endMs, startDate: null, endDate: null, content, tag, state: "completed" };
}

const beijing = (hour) => Date.UTC(2026, 8, 22, 16, 0) + hour * 3_600_000;
const entries = [
  entry(1, beijing(8), beijing(9), "本日工作"),
  entry(2, beijing(23), beijing(26), "跨午夜记录", "sleep"),
  entry(3, beijing(32), beijing(33), "次日记录", "leisure"),
];

const server = await createServer({
  configFile: false,
  logLevel: "error",
  plugins: [vuePlugin()],
  resolve: { alias: { vue: vueEntry } },
  server: { middlewareMode: true },
  optimizeDeps: { noDiscovery: true },
});

test("record history renders every record with the date jump control", async (t) => {
  t.after(() => server.close());
  const { default: RecordList } = await server.ssrLoadModule("/src/features/records/RecordList.vue");
  const html = await renderToString(
    h(RecordList, { pending: [], completed: entries, busy: false }),
  );

  assert.ok(html.includes("记录历史"), "应渲染历史标题");
  assert.ok(html.includes("跳转日期"), "应渲染与周视图一致的跳转日期控件");
  // 1.3.1 起日期改成自绘选择器，不再用原生 input[type=date]
  assert.ok(
    html.includes("date-picker-trigger") && html.includes("选择日期"),
    "跳转日期应为自绘的日期选择器",
  );
  assert.ok(
    html.includes("本日工作") && html.includes("跨午夜记录") && html.includes("次日记录"),
    "默认应显示全部已完成记录",
  );
  assert.ok(html.includes("2026-09-23 08:00"), "历史标题应包含北京时间日期");
  assert.ok(html.includes(">3<"), "标题计数应为全部记录条数");
  assert.ok(!html.includes("date-jump-reset"), "未选择日期时不应出现恢复全部按钮");
  for (const removed of ["month-rail", "history-month", "month-link"]) {
    assert.ok(!html.includes(removed), `月份时间条已移除：${removed}`);
  }
});
