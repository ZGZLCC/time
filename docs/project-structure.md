# 项目结构与代码功能

本文记录仓库当前已存在的目录和代码职责。桌面应用支持北京时间手动记录、四类标签、快速打卡、草稿补全、记录编辑删除，跨月跨年浏览的七天周视图和选中日期／本周／累计分类时长，按日期跳转记录历史，数据库备份、位置迁移与从备份恢复，三态主题，以及与网页版共用同步码的云同步；数据保存在本机 SQLite 数据库中。日期与时间不使用系统默认控件：日期是自绘日历，时间是小时与分钟两列的滚动选择器。

网页版不在本仓库：它随相册站点仓库 `Momo` 的 `web/` 目录维护，并随相册站点发布在 <https://zgzlcc.github.io/time>。桌面版与网页版共用同一个同步服务与同步码。

## 当前目录结构

```text
Time/
├─ AGENTS.md
├─ README.md
├─ .gitignore
├─ .gitattributes
├─ .dsh/
│  └─ skills/
│     └─ ponytail/
│        └─ SKILL.md
├─ .github/
│  └─ workflows/
│     └─ macos-build.yml
├─ index.html
├─ package.json
├─ package-lock.json
├─ tsconfig.json
├─ vite.config.ts
├─ scripts/
│  ├─ audit-databases.mjs
│  ├─ build-macos-package.sh
│  ├─ calendar.test.mjs
│  ├─ day-range.test.mjs
│  ├─ patch-nsis-installer.mjs
│  ├─ peek-cloud.mjs
│  ├─ proxy-fetch.mjs
│  ├─ record-list.test.mjs
│  ├─ time.test.mjs
│  ├─ verify-override-live.mjs
│  ├─ verify-sync-live.mjs
│  ├─ verify-tombstone-prune.mjs
│  └─ verify-upgrade.mjs
├─ docs/
│  ├─ AGENTS.md
│  ├─ DESIGN-webflow.md
│  ├─ implementation-plan.md
│  ├─ macos-验证清单.md
│  └─ project-structure.md
├─ src/
│  ├─ App.vue
│  ├─ api.ts
│  ├─ data-events.ts
│  ├─ day-range.ts
│  ├─ main.ts
│  ├─ style.css
│  ├─ sync.ts
│  ├─ tags.ts
│  ├─ theme.css
│  ├─ theme.ts
│  ├─ time.ts
│  ├─ components/
│  │  ├─ DateJump.vue
│  │  ├─ DatePicker.vue
│  │  ├─ DateTimeField.vue
│  │  ├─ TimeWheel.vue
│  │  ├─ calendar.ts
│  │  ├─ date-jump.css
│  │  └─ date-time.css
│  └─ features/
│     ├─ records/
│     │  ├─ RecordForm.vue
│     │  ├─ RecordList.vue
│     │  ├─ RecordsPage.vue
│     │  ├─ records-layout.css
│     │  └─ records.css
│     ├─ settings/
│     │  ├─ SettingsPage.vue
│     │  └─ settings.css
│     └─ week/
│        ├─ DayStats.vue
│        ├─ WeekExport.vue
│        ├─ WeekGrid.vue
│        ├─ WeekPage.vue
│        ├─ WeekStats.vue
│        ├─ week-grid.css
│        ├─ week-image.ts
│        └─ week.css
└─ src-tauri/
│  ├─ Cargo.toml
│  ├─ Cargo.lock
│  ├─ build.rs
│  ├─ tauri.conf.json
│  ├─ tauri.macos.conf.json
│  ├─ capabilities/
│  │  └─ default.json
│  ├─ icons/
│  │  ├─ icon.ico
│  │  ├─ icon.icns
│  │  ├─ icon.png
│  │  └─ icon.svg
│  ├─ windows/
│  │  ├─ hooks.nsh
│  │  └─ installer.nsi
│  ├─ migrations/
│  │  ├─ 0001_initial.sql
│  │  ├─ 0002_partial_entries.sql
│  │  ├─ 0003_sync_columns.sql
│  │  └─ 0004_empty_content.sql
│  ├─ src/
│  │  ├─ commands.rs
│  │  ├─ day.rs
│  │  ├─ error.rs
│  │  ├─ export.rs
│  │  ├─ lib.rs
│  │  ├─ main.rs
│  │  ├─ records.rs
│  │  ├─ week.rs
│  │  └─ db/
│  │     ├─ entries.rs
│  │     ├─ mod.rs
│  │     ├─ punch.rs
│  │     ├─ settings.rs
│  │     ├─ tests.rs
│  │     └─ week.rs
│  └─ tests/
│     ├─ empty_content.rs
│     ├─ migration4.rs
│     ├─ override_hides.rs
│     ├─ persistence.rs
│     ├─ restore.rs
│     ├─ roundtrip.rs
│     ├─ settings.rs
│     ├─ sync.rs
│     └─ week.rs
```

## 目录与文件职责

### 项目根目录与文档

| 路径 | 当前职责 |
| --- | --- |
| `AGENTS.md` | 项目规范入口，指向 `docs/AGENTS.md`、执行方案和 README 维护要求。 |
| `README.md` | 简述 1.3.3 已验证功能、检查方式与本机打包方式。 |
| `index.html` | Vite 页面入口；在加载界面前按已保存设置或系统外观确定亮暗主题。 |
| `package.json` | npm 命令：开发服务器、类型检查、前端构建、四项 Node 检查（北京时间、单日区间、记录历史渲染、日历计算）及 Tauri 桌面开发与打包。 |
| `package-lock.json` | 锁定 npm 依赖。 |
| `.gitattributes` | 统一以 LF 提交与检出，避免 shell 脚本在 macOS 上因 CRLF 无法执行；图片与数据库文件按二进制处理。 |
| `tsconfig.json` | 严格检查 `src/` 中的 TypeScript 和 Vue 文件。 |
| `vite.config.ts` | 配置 Vue 插件、开发服务器及 Rust 构建目录监视排除项。 |
| `.dsh/skills/ponytail/SKILL.md` | 项目级本地技能 `ponytail`：约束编码任务优先取最省实现（YAGNI、标准库、原生能力），支持 `lite`／`full`（默认）／`ultra` 三档强度；只约束实现方式，不改变业务行为。 |
| `.github/workflows/macos-build.yml` | 在 GitHub Actions 的 `macos-15`（Apple Silicon）上构建 `aarch64-apple-darwin` 产物：Node 22、Rust 稳定版加缓存、`tauri build --bundles app`，用 `ditto` 把 `Time.app` 压成 zip 上传；`.dmg` 单独尝试，失败不阻塞。只在 `main` 推送（跳过纯文档改动）或手动触发时运行。 |
| `scripts/time.test.mjs` | 检查北京时间转换、日期边界、整分钟时长显示、周标题与完整日期标题格式、闰日与跨年周跳转。 |
| `scripts/day-range.test.mjs` | 检查北京时间单日区间、跨 UTC 日界的日期归属、跨午夜记录是否计入某一天，以及空日期时不过滤记录。 |
| `scripts/record-list.test.mjs` | 用 Vite SSR 渲染记录历史组件，检查默认列出全部记录、标题含北京时间日期，以及跳转日期控件已替换月份时间条。 |
| `scripts/calendar.test.mjs` | 检查日期选择器的日历计算：周一起始、42 格连续、闰年与二月天数、非法日期、跨月跨年加减。 |
| `scripts/verify-upgrade.mjs` | 把真实数据库复制到隔离目录，按应用的逻辑走一遍「复制成新库 → 迁移」，逐条比对记录并确认原库一个字节都没变。 |
| `scripts/audit-databases.mjs` | 只读核对原库与新库：按 `public_id` 确认没有任何原库记录丢失，并列出被后续编辑改动的记录与新库多出的部分。 |
| `scripts/verify-sync-live.mjs` | 用真实 Worker 与同步码，在数据库副本上跑一遍桌面版同步流程，验证 Rust 合并逻辑与 Worker 契约能对接（需设置 `TIME_SYNC_CODE`）。 |
| `scripts/verify-override-live.mjs` | 线上验证两种覆盖方向：本地删除后上传云端也消失、云端独有记录在「上传本地记录」后消失、「从云端恢复」能把云端内容整体落下。 |
| `scripts/verify-tombstone-prune.mjs` | 线上验证墓碑保留期：过期墓碑被云端丢掉、保留期内的留下，并清理探针。 |
| `scripts/peek-cloud.mjs` | 查看某个同步码下云端存了什么，排查同步问题用。 |
| `scripts/proxy-fetch.mjs` | 代理感知的 HTTPS 请求工具（环境变量与 Windows 系统代理），供验证脚本复用。 |
| `scripts/patch-nsis-installer.mjs` | 给 `src-tauri/windows/installer.nsi` 的 `PageReinstall` 加上“已有安装则跳过维护选项页”的判定，可重复执行，模板缺少锚点时明确报错。 |
| `scripts/build-macos-package.sh` | 在 macOS 用已构建的 `Time.app` 制作按 bundle ID 原位置升级的 `.pkg`，并移开构建副本避免安装器误识别。 |
| `docs/AGENTS.md` | 项目开发、验证、可访问性和文档维护规范。 |
| `docs/DESIGN-webflow.md` | 前端视觉规范，包括配色、字体、间距、圆角、组件和响应式规则。 |
| `docs/implementation-plan.md` | 产品技术方案、数据约束、流程及分阶段验收标准。 |
| `docs/macos-验证清单.md` | 给测试者的 macOS 操作单：两种系统版本的打开方式（含未签名应用的一次性授权）、14 项验收清单、数据位置、反馈格式与已知限制。 |
| `docs/project-structure.md` | 当前目录与职责说明；项目代码或文档变化时同步维护。 |

### `src/` 前端

| 路径 | 当前职责 |
| --- | --- |
| `src/main.ts` | 创建 Vue 应用，加载全局、页面和主题样式并挂载到页面。 |
| `src/App.vue` | 应用根组件，提供记录、周视图、设置三个页面导航，并接收周视图的编辑入口。 |
| `src/api.ts` | 定义传输类型；封装记录、打卡、列表、本周与单日统计、图片导出、数据库设置、备份恢复和云同步的 IPC。`database_directory` 与 `move_database` 仍保留在后端与这里，当前界面不再调用。 |
| `src/data-events.ts` | 本地数据变更广播；记录页被 `KeepAlive` 缓存，靠它在下一次同步、恢复或清空后重新读取。 |
| `src/tags.ts` | 统一维护四类标签的值、中文名称和强调色。 |
| `src/time.ts` | 在北京时间输入／显示和 UTC Unix 毫秒之间转换，并提供固定时区的日期跳转、周标题、完整日期标题及整分钟时长格式化。 |
| `src/day-range.ts` | 计算北京时间某一天的起止毫秒、把毫秒转换为北京时间日期、判断记录是否出现在该日（含跨午夜记录）并按日筛选记录列表。 |
| `src/style.css` | 全局字体、重置、页面布局、亮色设计变量、导出图片用的固定标签色和通用按钮样式。 |
| `src/theme.ts` | 读取与保存主题选择，跟踪系统外观变化并应用亮暗主题。 |
| `src/theme.css` | 暗色主题的画布、卡片、表单、边框、文字和标签设计变量。 |
| `src/sync.ts` | 云端同步：读写本机同步配置与上次同步／导出时间，请求 Worker 的 `/api/entries`，配合 Rust 侧合并完成「立即同步」「上传本地记录」「从云端恢复」，并把连不上、同步码失效转成中文提示。 |
| `src/components/calendar.ts` | 日历纯计算：周一起始的月份网格、加减天／月（日号夹到目标月）、非法日期拒绝。 |
| `src/components/DatePicker.vue` | 自绘日期选择：触发按钮显示 `YYYY-MM-DD`，弹出七列日历（固定 6 行、今天带边框、选中加深加粗），按触发按钮的实际坐标摆放并在空间不足时向上翻转，底部只有“今天”按钮，支持方向键与点击外部关闭。 |
| `src/components/TimeWheel.vue` | 滚动时间选择器：小时与分钟两列竖条，列表渲染三份并在滚到外侧时平移回中间，做出无限循环（59 后面是 0、23 后面是 0）；上下内边距等于一条选中带高度，让首末项也能滚到正中；支持滚轮、点击与方向键。 |
| `src/components/DateTimeField.vue` | 一组「日期 + 时间」：日期用 `DatePicker`，时间用 `TimeWheel`，触发按钮显示 `HH:MM`（未设置时显示 `--:--`），打开时默认落在当前时刻。 |
| `src/components/date-time.css` | 日期选择与时间滚轮的样式，沿用项目的边框、圆角与配色变量。 |
| `src/components/DateJump.vue` | 复用“跳转日期”输入与可选“全部”按钮，向父组件抛出日期选择与恢复全部事件。 |
| `src/components/date-jump.css` | 跳转日期标签、日期输入和恢复按钮的对齐与控件样式。 |
| `src/features/records/RecordsPage.vue` | 记录页状态协调：读取记录与待补全、独立开始时间打卡、常驻手动表单、保存、居中删除确认、错误反馈，并在写入成功后广播数据变化。 |
| `src/features/records/RecordForm.vue` | 分开的开始／结束日期与时间输入、同日默认、可缺字段草稿、内容字数提示、标签按钮及编辑补全。 |
| `src/features/records/RecordList.vue` | 待补全记录以固定宽度占位符显示起止时间；历史默认显示全部已完成记录并带起止时间标题，最多完整显示五条，可用跳转日期只查看某一天（含跨午夜记录）并恢复全部；内容为空时显示说明文字。 |
| `src/features/records/records.css` | 记录列表与表单样式、历史单行时间、标签配色、焦点状态和响应式规则。 |
| `src/features/records/records-layout.css` | 记录页卡片与历史布局、待补全和历史的滚动窗口、历史标题栏的跳转日期控件、全宽标签按钮和居中删除确认框。 |
| `src/features/week/WeekPage.vue` | 周导航、选中日期／本周／累计统计和记录详情协调；切换周或跳转日期时同步重新读取该日统计；点击时间块后滚动并聚焦到上方以时间为标题、事情为正文的详情。 |
| `src/features/week/WeekGrid.vue` | 随窗口宽度适配的七天 00:00–24:00 时间轴、跨日记录片段和图片导出入口；事件块按起止分钟定位并保留最小可点高度。 |
| `src/features/week/DayStats.vue` | 展示选中日期（今天时标注“（今天）”）的四类标签时长，样式与本周卡片一致，不显示合计。 |
| `src/features/week/WeekStats.vue` | 展示本周或累计的四类标签时长，精度到分钟。 |
| `src/features/week/WeekExport.vue` | 管理导出地址、系统文件夹选择、PNG 生成和保存反馈。 |
| `src/features/week/week-image.ts` | 用 Canvas 绘制七天时间表与本周四类时长；标签色取运行时的固定导出变量（亮暗主题相同），保证同一份数据在不同主题下导出结果一致；文字不传 `fillText` 的 `maxWidth`，靠裁剪而非压缩限制宽度。 |
| `src/features/week/week.css` | 周视图导航、统计、详情及供设置页复用的文件夹控件样式。 |
| `src/features/week/week-grid.css` | 自适应七天网格、小时刻度与分类事件色带样式。 |
| `src/features/settings/SettingsPage.vue` | 四张整行设置卡片：云端同步（同步码、立即同步、上传本地记录、从云端恢复）、备份与恢复（导出备份、从文件恢复、清空记录）、本地数据概览、修改主题；覆盖类的同步、恢复与清空都走居中确认框。 |
| `src/features/settings/settings.css` | 设置卡片布局、全宽三按钮主题选择及窄窗口适配。 |

### `src-tauri/` 桌面与 Rust 后端

| 路径 | 当前职责 |
| --- | --- |
| `src-tauri/Cargo.toml` | Rust 包配置，依赖 Tauri 2、bundled SQLite 的 `rusqlite`、Serde 与官方文件夹对话框插件。 |
| `src-tauri/Cargo.lock` | 锁定 Rust 依赖。 |
| `src-tauri/build.rs` | 生成 Tauri 构建资源。 |
| `src-tauri/src/main.rs` | 启动桌面应用并委托给 `time_lib::run()`。 |
| `src-tauri/src/lib.rs` | 初始化应用数据目录、创建数据库状态、注册文件夹对话框插件，并登记全部 IPC 命令。 |
| `src-tauri/src/commands.rs` | 记录、打卡、列表、本日与周统计、图片保存、数据库设置、备份恢复及云同步命令的 IPC 边界。 |
| `src-tauri/src/export.rs` | 校验 PNG 字节、导出文件夹及周日期，保留重名旧文件并写入新图片。 |
| `src-tauri/src/error.rs` | 统一错误码和中文错误消息，涵盖校验、重叠、数据库、迁移和系统时钟异常。 |
| `src-tauri/src/records.rs` | 记录标签、状态、可缺字段输入／输出类型，以及草稿与已完成记录的校验（内容可为空、最多 500 字）。 |
| `src-tauri/src/day.rs` | 复用周区间裁剪规则得到北京时间单日 `[00:00, 24:00)` 视图，供本日统计使用。 |
| `src-tauri/src/week.rs` | 校验日期并计算北京时间周一至下周一边界；按左闭右开区间裁剪每日片段，统一累加四类毫秒时长。 |
| `src-tauri/src/db/mod.rs` | 管理 SQLite 连接、可切换位置的应用状态与事务性迁移（版本必须逐个递增），升级前备份、失败回滚；对外暴露同步与设置的封装方法。 |
| `src-tauri/src/db/settings.rs` | 数据库位置解析与「旧库复制成新库」的一次性升级；生成不覆盖旧文件的在线备份、切换位置，以及从备份文件恢复（校验 schema 版本后整体替换，只读打开备份文件）；另含去掉 Windows 扩展长度前缀的 `friendly_path`。位置切换保留在后端，当前界面未提供入口。 |
| `src-tauri/src/db/entries.rs` | 以参数化 SQL 实现新增、编辑、列表、软删除、重叠检查与清空；另含同步用的 `list_all_for_sync`、`merge_cloud_entries`（按 `public_id` 对应、`updated_at` 后写胜出，并挑出需要推回云端的更晚版本）、`mirror_cloud_entries`（用云端快照整体覆盖本地，快照里没有的记录打上删除标记）以及按保留期真正删除墓碑的 `purge_tombstones`。 |
| `src-tauri/src/db/punch.rs` | 以即时事务为每次打卡新增仅含开始时间的草稿，并按 `start_ms` 倒序查询待补全记录。 |
| `src-tauri/src/db/week.rs` | 以区间相交条件查询已完成记录，供选中周和单日统计复用；汇总全部已完成记录的分类时长。 |
| `src-tauri/src/db/tests.rs` | 检查迁移失败时事务回滚且原数据和升级前备份保留，以及升级保留旧记录并把 `running` 转为草稿。 |
| `src-tauri/migrations/0001_initial.sql` | 创建初始记录表、状态／标签／时间约束、唯一进行中记录索引和时间查询索引。 |
| `src-tauri/migrations/0002_partial_entries.sql` | 保留旧记录并扩展可空起止时间和日期字段，将旧进行中记录转为草稿。 |
| `src-tauri/migrations/0003_sync_columns.sql` | 为云同步补 `public_id`／`updated_at`／`deleted_at` 三列；用 SQLite 自带的 `randomblob`／`hex` 回填 v4 UUID 并把 `updated_at` 初始化为按时间递增，再建部分唯一索引（只约束非空 `public_id`）。 |
| `src-tauri/migrations/0004_empty_content.sql` | 重建记录表，把「内容至少 1 字」的检查改成「内容不超过 500 字」，让空内容的已完成记录可以保存；同时保留标签、时间与状态约束并重建索引。 |
| `src-tauri/tests/empty_content.rs` | 空内容的已完成记录：保存、列表、快照往返、计入统计，以及从另一台设备（空库 + 云端一条空内容记录）合并进本机。 |
| `src-tauri/tests/migration4.rs` | 在真实数据库副本上验证迁移 4 逐条保住记录、字段不变。 |
| `src-tauri/tests/override_hides.rs` | 用真实数据库副本复现「用云端覆盖本地后，本机独有记录不应再出现在可见列表」。 |
| `src-tauri/tests/persistence.rs` | 检查 CRUD、重叠规则、打卡草稿在重启后恢复，以及残缺草稿留在待补全而不进入已完成历史。 |
| `src-tauri/tests/restore.rs` | 检查从备份整体替换当前记录、导出路径去掉 Windows 前缀且是普通绝对路径、旧 schema 备份被明确拒绝且不动当前数据、文件缺失或不是数据库时安全失败。 |
| `src-tauri/tests/roundtrip.rs` | 用内存里的「云端」模拟 Worker 合并规则，验证两种覆盖方向都真的覆盖，且再次覆盖不会复活已删记录。 |
| `src-tauri/tests/settings.rs` | 检查在线导出不覆盖同名文件，以及位置迁移后记录数不变、原文件保留。 |
| `src-tauri/tests/sync.rs` | 检查新记录生成唯一 `public_id`、软删除保留墓碑、云端记录合并（后写胜出、待推送判定、非法记录跳过、同快照往返不丢数据）、镜像覆盖的各种边界，以及墓碑按保留期真正删除。 |
| `src-tauri/tests/week.rs` | 检查周起始日期、跨年／跨午夜／跨周裁剪、四类毫秒汇总、本日统计裁剪与草稿排除、累计时长排除草稿及已完成记录查询边界。 |
| `src-tauri/tauri.conf.json` | 配置应用标识、主窗口、开发地址、构建路径，以及支持两种安装范围的 Windows NSIS 安装包和图标。 |
| `src-tauri/windows/hooks.nsh` | 安装前读取 HKLM／HKCU 的旧安装登记，仅在一个范围存在旧安装时自动切换范围，并按 `RestorePreviousInstallLocation` 得到的原目录覆盖安装。 |
| `src-tauri/windows/installer.nsi` | 基于 Tauri CLI 2.11.x 的 NSIS 模板快照，并加入“已有安装时跳过维护选项页”的判定；在 `tauri.conf.json` 中通过 `bundle.windows.nsis.template` 使用，构建时替换 `{{...}}` 占位符。 |
| `src-tauri/tauri.macos.conf.json` | macOS 自动合并的构建配置：生成 `.app`／`.dmg`，使用 PNG／ICNS 图标，声明最低系统版本 macOS 11.0，本机测试使用临时签名。 |
| `src-tauri/capabilities/default.json` | 配置主窗口 Tauri capability，仅开放系统文件夹选择对话框权限（`dialog:allow-open`）。 |
| `src-tauri/icons/icon.ico` | 从同源 SVG 生成的多尺寸 Windows 桌面应用图标。 |
| `src-tauri/icons/icon.icns` | 从同源 SVG 以 1024 抗锯齿渲染后生成的 macOS 应用包图标，含 16～1024 各档尺寸。 |
| `src-tauri/icons/icon.png` | 从同源 SVG 渲染的 512×512 macOS 窗口图标。 |
| `src-tauri/icons/icon.svg` | 黑色圆环、蓝色时针的图标原稿，同时用于应用内顶部标识。 |

## 当前调用流程

1. `index.html` 应用持久化主题后加载 `src/main.ts`，入口创建 `App.vue`，在记录、周视图和设置页之间切换；记录页用 `KeepAlive` 缓存，切回时重新读取数据。
2. 记录页通过 `list_entries` 和 `list_pending` 恢复历史及待补全内容，在卡片内按所选日期筛选本地已完成记录；打卡、保存、删除成功后广播数据变化，让其他页面跟着刷新。
3. 周视图调用 `get_week`、`get_day_totals` 和 `get_all_totals` 分别读取本周、选中日期与全部已完成记录的分类时长，并可按系统 Downloads 路径或所选文件夹导出 PNG。
4. 周查询以北京时间周边界筛选已完成记录，计算跨午夜的逐日显示片段和按标签裁剪的时长；单日查询复用同一裁剪逻辑，只取选中日期 00:00～24:00，`get_day_totals` 先取当天区间再交给同一套装配函数。
5. 时间块滚动并聚焦到表格上方的完整详情；编辑入口切换到记录页右侧表单。
6. 设置页通过 IPC 导出 SQLite 在线备份、复制并校验后切换数据库位置、从备份文件整体恢复或软删除全部记录；当前路径写入应用数据目录，重启后恢复。
7. 云同步用同步码换一份云端数据：`cloud_snapshot` 取本地全量（含墓碑）推给 Worker，「立即同步」把云端快照交给 `merge_cloud` 逐条合并，「上传本地记录」带 `?replace=1` 整体覆盖云端，「从云端恢复」用 `mirror_cloud` 让本地与云端一致；写入成功后广播数据变化并清理过期墓碑。
8. 成功写入后页面更新对应记录；失败时保留输入并显示错误。删除、覆盖类同步、恢复和清空都先用居中确认框说明后果。

## 构建产物与当前范围

- `node_modules/`、`dist/` 与 `src-tauri/target/` 是依赖或构建产物，由 `.gitignore` 排除。Windows 发布程序位于 `src-tauri/target/release/time.exe`，NSIS 安装包位于 `src-tauri/target/release/bundle/nsis/Time_1.3.3_x64-setup.exe`。
- Windows 安装器按 `tauri.conf.json` 的 `installMode: "both"` 支持当前用户与全用户范围，并在检查到已有安装时跳过维护选项页、默认沿用原范围与目录覆盖升级。
- macOS 产物由 `.github/workflows/macos-build.yml` 在 GitHub Actions 的 `macos-15`（Apple Silicon）上构建：免安装的 `Time.app` 压缩包与 `.dmg` 均产出，另有基于 Apple `productbuild` 的 `.pkg` 可选路线；应用声明最低支持 macOS 11.0，使用临时签名，首次打开需在系统设置里允许一次。原生包的启动、记录持久化、图片与备份导出仍需在 Apple Silicon Mac 上完成实机验证。
- 已完成记录使用 UTC Unix 毫秒存储，表单和界面按北京时间显示；待补全记录可保留未填写具体时间的日期，打卡记录只保存开始时间；内容为空的已完成记录按完整记录处理。
- 记录编号除本地整数主键外还有跨端稳定的 `public_id`，同步按 `updated_at` 做后写胜出、用 `deleted_at` 墓碑传递删除；墓碑超过保留期后由本地与云端分别真正删除。
- 当前界面支持四类标签按钮、手动记录、快速打卡、草稿、历史编辑删除与按日期跳转、按周浏览和定位详情、选中日期／本周／累计分类时长统计、周时间表 PNG 导出、数据库备份与位置迁移、从备份恢复与清空记录、三态主题，以及云端同步。
- 周视图按 `[start, end)` 查询和裁剪，只纳入已完成记录；待补全数量独立显示，选中日期、本周与累计的四类时长以毫秒汇总并在界面显示到分。
- 记录历史按北京时间日期展示，默认显示全部已完成记录；选择日期后只显示该日记录，跨午夜记录计入它覆盖的每一天。
