// 让 NSIS 安装器在检测到已有安装时跳过“维护选项”（Already Installed）页。
// Tauri 用普通 `Page custom PageReinstall` 声明该页，MUI 的页面回调不生效，
// 因此需要在生成的 installer.nsi 里为 PageReinstall 增加一段前置判定。
// 该脚本可重复执行：已打过补丁时直接跳过。
import { readFile, writeFile } from "node:fs/promises";

const target = process.argv[2];
if (!target) {
  console.error("用法：node scripts/patch-nsis-installer.mjs <installer.nsi 路径>");
  process.exit(1);
}

const MARKER = "tauri-hook: already installed";
const ANCHOR = "Function PageReinstall";
const PATCH = `${ANCHOR}
  ; ${MARKER}: skip the maintenance page and upgrade in place
  ReadRegStr $R0 SHCTX "\${UNINSTKEY}" ""
  ReadRegStr $R1 SHCTX "\${UNINSTKEY}" "UninstallString"
  \${If} "$R0$R1" != ""
    StrCpy $ReinstallPageCheck 1
    Abort
  \${EndIf}

`;

const script = await readFile(target, "utf8");
if (script.includes(MARKER)) {
  console.log(`installer.nsi 已有跳过维护选项页的补丁：${target}`);
  process.exit(0);
}
if (!script.includes(ANCHOR)) {
  console.error(`未找到 ${ANCHOR}，NSIS 模板可能已变化，未能应用跳过维护选项页的补丁：${target}`);
  process.exit(1);
}

await writeFile(target, script.replace(ANCHOR, PATCH), "utf8");
console.log(`已为 installer.nsi 应用跳过维护选项页的补丁：${target}`);
