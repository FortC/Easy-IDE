# Changelog

本项目所有显著变更均记录于此，格式遵循 [Keep a Changelog](https://keepachangelog.com/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [0.2.5] - 2026-10-13

### Added
- **代码配色主题**：设置 → 通用新增「代码配色」选择器，内置 5 套语法配色 + 「跟随界面」，色块卡片即点即换（零重建换肤）：
  - 深邃黑（深色，VS Code Dark+ 风）/ 深蓝夜航（深色，Tokyo Night 风）
  - 亮白清爽（浅色，GitHub Light 风）/ 暖纸护眼（浅色，Solarized 风）
  - 高对比度（纯黑底 + 高亮加粗语法色，弱视友好）
  - 「跟随界面」默认：深色界面用深邃黑、浅色界面用亮白清爽，切换界面主题时代码配色自动联动。
  - 实现：`lib/editorThemes.ts` 主题注册表产出 `--code-*` CSS 变量挂编辑器宿主，高亮样式与编辑器 chrome 统一引用变量，换肤只重设变量；`settings.code_theme` 持久化（Rust 侧字段向前兼容，旧配置文件无该字段自动回落 auto）。md 笔记保持原有默认高亮，不受影响。

## [Unreleased]

（暂无）

## [0.2.6] - 2026-10-15

### 复核升级轮：功能 / 界面 / 动画 / 操作感

#### Added
- **包体门禁 `npm run verify`**：`scripts/check-bundle.mjs` 解析 dist/index.html 实际首屏引用，对照 `docs/performance-budget.json` 判定超标即失败；`check_i18n.py` 修复失效的绝对路径并接入。
- **基础交互原语 6 件**（`components/common/`）：BaseButton（变体 + 按压反馈）、BaseModal（焦点陷阱 + Esc 全局监听 + 关闭归还焦点 + `role="dialog"`）、BaseTabs（`role="tablist"` + 左右方向键 + 徽标）、BaseSegmented（radiogroup + 方向键）、BaseSwitch、BaseTooltip（延迟 + 定向）。InputDialog / NewNoteDialog 已迁移 BaseModal。
- **标签页拖拽排序**：编辑器展开态网格卡片支持 HTML5 拖拽重排，左/右半边落点指示（inset box-shadow，不动尺寸），拖拽期间全局禁过渡。
- **控制台输出虚拟滚动**：自研 `lib/useVirtualList.ts`（行高 ResizeObserver 实测，兼容字体缩放），命令头与输出行打平统一虚拟化；新增「回到底部」悬浮钮，离底暂停自动跟随、贴底恢复。
- **搜索模糊匹配**：Rust 侧 `fuzzy_score` 子序列打分（驼峰缩写 `gt`→`getTitle`、连续/词首优先），作为 rank=1000 段排在精确命中之后；全文结果高亮命中词；新增检索 loading（三点脉冲）与失败提示。
- **小计面板入口**：ActivityBar 补「小计」入口（此前 `leftTab==='jots'` 渲染分支存在但不可达）。
- **全局错误兜底 `lib/errorGuard.ts`**：Vue errorHandler + unhandledrejection + window.onerror 统一 toast（10s 窗口限 3 条、同类去重），消灭「点了没反应」式静默失败。
- **懒加载挂起占位**：16 个 defineAsyncComponent 挂 loadingComponent（h() 渲染的三点脉冲）。
- **令牌补齐**：`--space-1..5`、`--z-base..toast`、`--dur-loop` / `--anim-loop` / `--anim-ease-in-out`、`--text-warning`、`--window-close`；全局 `:focus-visible` 焦点环；`select` 元素统一基座样式。

#### Fixed
- **控制台拖拽不跟手**：`startDrag` 漏挂 `body.is-resizing` 且 `.cs-panel` 带 `transition: height`，拖动全程在追赶 180ms 过渡。已挂标记并移除 height 过渡（红线2）。
- **Ctrl+D 双触发**：CodeMirror defaultKeymap 的 `Mod-d`（选中下个相同文本）与全局「日记」同时触发。日记改绑 **Ctrl+Shift+D**，并新增 `CM_RESERVED_SEQS` 机制。
- **全局快捷键不判事件源**：输入框 / textarea / contenteditable 内不再劫持（修「AI 输入框按 Ctrl+P 弹文件面板」）；双击 Shift 手势同样跳过输入框。
- **动效强度「精简」档形同虚设**：仅点名 8 个 Vue 过渡类，管不住 47 处 scoped 过渡与 6 个 infinite 动画。改为 `*` 通配压时长 + 禁循环 + 只留 opacity/颜色过渡。
- **旋转 spinner 违例**：删除 AgentPanel 死代码 `ag-spin`；全部执行中指示统一三点脉冲。
- **16 处硬编码时长 + 2 处裸 ease-in-out** 全部收敛到 `--anim-loop` / `--anim-ease-*` 令牌。
- **分隔条 `transition: width` 违红线**：EditorPane 分屏分隔条改 inset box-shadow 表达加粗。
- **19 处硬编码错误色**（`var(--text-error, #xxx)` fallback 与 `#e81123` 等）全部回收令牌。

#### Changed
- **首屏瘦身达标**：主包 JS 269.7 → **241.5 KB**、首屏 CSS 87.9 → **71.1 KB**（红线 ≤80KB）。设置面板 / 搜索面板 / 增量包 / 配方卡片改懒加载；xterm CSS 随终端 JS 一起按需注入。
- **高频交互按压反馈补齐**：菜单项（键盘 ↑↓ 高亮 + Enter 直选 + 焦点归还）、文件树行、控制台页签与工具钮、标题栏窗口钮、右栏 tab（迁移 BaseTabs）。
- **面板分隔条键盘可调**：左右栏 / 控制台高度 resizer 支持 Tab 聚焦后方向键步进（Shift=40px）、Home 复位、双击复位默认宽。
- **无障碍补底**：文件树 `role="treeitem"` + `aria-expanded/selected`、ActivityBar `aria-label/pressed`、搜索输入 combobox + `aria-activedescendant`、toast `aria-live="polite"`、状态栏 `role="status"`。
- **死代码清理**：anim.css 中对 scoped 组件无效的全局按压选择器（rt-btn/tb-btn/oc-card）、失效的 `.rt-btn` / `.st-tab` / `.st-seg` / `.cs-badge` 等。
- Rust 单测新增 fuzzy_score 3 项（现 97 通过 + 1 项 ignored 基准）。

## [0.2.4] - 2026-10-13

依据《总体优化方案v1.md》完成的两轮改造：语境模型 + 导航重构 + AI 融合 + 动效体系 + 性能瘦身。

### Added
- **语境模型（双语境抽象）**：左栏面板 + 打开文件类型双因子判定 `dev` / `note`，驱动右栏 tab 集、RunBar 显隐、底部面板页签、AI 快捷动作集的联动。切换语境时校正右栏 tab（不在新集合内回落 AI），**不重置 AI 会话**。
- **Activity Bar（48px 纯图标导航）**：6 个语义单一入口（项目结构 / 文档中心 / 版本控制 / Maven / 笔记 / 标签），hover 出 tooltip，左侧 2px 选中指示条（只动 transform）。点当前入口可收起左栏。
- **引用查找面板**（开发语境）：以当前文件最外层符号（class/interface/enum/record）为锚点，跨文件检索同名符号，点击跳转定位。
- **AI 记忆面板**（开发语境）：项目级记忆存 `.easyide/memories.json`，记录项目约定供 AI 参照。
- **底部「问题」页签**：从运行输出自动抽取诊断（兼容 Maven `[ERROR] x.java:[12,5] msg` 与 Gradle `x.java:12: error: msg`），带错误计数徽标；仅代码语境显示。
- **AI 选区上下文（AI5）**：编辑器选中代码自动挂载到 agent store，AI 面板输入框显示引用条（含文件名与行号），**代码模式下 Ctrl+I 唤起 AI 面板**即可直接提问。提问时上下文以结构化文本注入，AI 无需再 `read_file` 找回来（省一轮往返与 token）。注入一次即清空。
- **AI 快捷动作 chips**：AI 面板顶部语境化快捷动作（代码 7 个 / 笔记 5 个），与 RunBar 共用 `stores/actions.ts` 的同一份定义。
- **动效强度设置**：设置 → 常规 → 动效强度 `[完整 / 精简 / 关闭]`，挂 `data-motion` 到 `<html>`，与系统 `prefers-reduced-motion` 双保险。
- **冷启动分段埋点**：Rust 侧注入 `__easyideTauriStart`，前端记录「导航→脚本」「导航→挂载」「Rust→挂载」「App 挂载完成」四段耗时至 `localStorage.easyide-boot-breakdown`。
- **栏宽偏好持久化**：左右栏宽度记入 localStorage，重启后保持。

### Changed
- **首屏主包从 1,117.66 KB 降到 229.80 KB（−79.4%）**，gzip 395.46 KB → 76.40 KB（−80.7%）。dist 总体积 2.0 → 2.1 MB（换来首屏 −79%）：
  - `markdown-it` 拆出：`lib/markdown/renderer.ts` 拆为纯文本工具（零依赖）+ 新增 `lib/markdown/md-render.ts`（渲染管线）；预览与 AI 消息按需懒加载。
  - `yaml` 改动态 `import()`（`lib/tags.ts`）。
  - `BacklinksPanel` / `PropertiesPanel`（笔记语境专属，代码场景永不打开）、`DocsPanel` / `VcsPanel` / `MavenPanel`（重 IO 面板）全部改 `defineAsyncComponent`。
  - `vite.config.ts` 加函数式 `manualChunks`，编辑器内核 / 框架 / markdown 三块 vendor 独立成 chunk。
  - `theme.css` 拆出 `anim.css`（动效与通用交互），首屏 CSS 90.13 KB → 77.68 KB。
- **导航重构**：`Ribbon.vue`（64px、图标+文字、10 个模块混编「面板切换 + 全幅视图 + 主题」）替换为 `ActivityBar.vue`（48px 纯图标、6 个单一语义入口）；清理 25 个 `rb.*` 死键与 jot 空态旧文案。
- **图谱 / 日历降级为右栏 tab**：原为全幅视图（整块替换工作区，导致左右栏消失、布局跳变）；现为右栏 tab，与左栏面板互不干扰。
- **底部面板升格**：从 `consoleVisible` 二态改为「输出 / 终端 / 问题」三页签常驻结构，可折叠（收起仅留页签条）。
- **AI 入口统一**：删除 `AiDialog.vue`（独立弹窗），编辑器右键菜单、选区悬浮按钮、文档中心「问 AI」全部导向右侧 AI 面板。**4 套入口收敛为 1 个中心**。
- **RunBar 改薄壳**：动作定义与提示词外移到 `stores/actions.ts`，与 AgentPanel 的 chips 同源，不再两处重复维护。
- **动效令牌体系化**：新增 `--anim-ease-out/in/spring`、`--dur-instant/fast/base/slow` 与语义别名 `--anim-hover/press/enter/leave`，替换散落的 130/150/170/90/80ms 硬编码。旧别名保留兼容。
- **AI 默认成为右栏首个可见 tab**（原默认 `outline`），落实「深度融合 AI」的产品定位。
- **全文搜索快捷键改为双击 Shift**（IDEA 式）：两次纯 Shift 按下、间隔 400ms 内且中间无其他按键即唤起搜索面板；标题栏搜索框提示同步改为「双击 Shift」。Ctrl+Shift+F 仍作为可改绑的备用键保留。
- **图谱 / 日历回归左侧大模块**（修正本轮前一条「降级为右栏 tab」的做法）：GraphView/CalendarView 自带完整布局（图谱含 240px 侧栏），320px 右栏装不下。现由活动栏入口进入主区全幅视图，左右栏保持可见；右栏 tab 集恢复为「AI · 大纲 · 链接类 · 元信息类」。
- **Activity Bar 悬浮反馈**：hover 时图标淡出、模块名淡入（纯 opacity 交叉淡入，符合动效红线），8 个大模块入口 + 主题/设置均有短名。
- **RunBar 终端按钮加文字**：图标旁显示「终端」，悬浮提示「弹出终端面板」，点击直落终端页签（与按钮文案一致）。
- **首屏 CSS 瘦身**：「系统打开文件」模式弹窗拆为懒加载组件 `OpenChoiceDialog.vue`（约 2.5KB 样式移出首屏包），首屏 CSS 回到 ≤80KB 预算内（78.41 KB，主包 JS 233.23 KB）。

### Fixed
- **布局跳变（串场）**：`ui.fullView` 概念废除，`showLeft` 内的 `if (!sameModule || ui.fullView) ui.view = "editor"` 特判补丁删除。图谱/日历切换不再顶掉左栏与 RunBar。
- **交互「不跟手」**：全局补 `:active` 按压反馈（`transform: scale(0.96)`，走合成器立即生效），覆盖按钮 / 导航 / 右栏 tab / 文件树与列表项；scoped 组件内单独补齐。此前所有交互只有 hover 反馈，点下去没有按下感。
- **工具调用不可读**：从纯文本工具名改为语义图标 + 中文标签 + 步骤序号 + 完成/拒绝徽标。
- **文件树「展不开」体感**：展开空目录 / 加载失败时此前都显示一片空白。现空目录显示「（空目录）」、失败显示「加载失败，点击重试」（可重试）。注：`code/epointbhcs/src/main/java` 在 SVN 仓库中即为空目录，Java 源码实际位于 `code/epointbhcs-parent/*/src/main/java/com/...`。
- **AI 记忆面板样式失效**：模板类名为 `mm-*` 而样式块写的是 `mem-*`（除根节点外全部错位），面板自 0.2.4 起无样式裸奔。已统一为 `mm-*`。
- 清理 Ribbon 时代死样式 `.rb-btn`（anim.css）。
- **执行中反馈**：旋转 spinner 改为三点脉冲（重绘更少、视觉噪音更低），并尊重 `prefers-reduced-motion`。
- 右栏 tab 切换的上浮位移在窄栏内略显晃动，改为纯淡入。
- AI 面板动效：工具调用结果卡入场、命令确认门滑入（吸引注意）；流式输出容器不加入场动画，避免每帧重放造成闪烁。
- 异步加载面板闪现：新增骨架屏 `.async-skeleton`（`background-position` 动画，不触发重排）。
- 收起态底部面板拖拽时高度计算基准错误：拖拽开始前先展开。

### 规范
- `AGENTS.md` 新增**语境模型**（明确反链/属性为笔记语境专属、AI 动作集共用定义）、**动效七条红线**（含 scoped 样式需单独补按压反馈的坑）、**AI 集成约定**（单一入口、选区上下文、Ctrl+I 不得与斜体冲突）；布局章节按已落地的 ActivityBar 如实更新，并写入首屏包体硬指标。

## [0.2.3] - 2026-10-12

### Added
- **系统托盘**：托盘图标 + 右键菜单（显示主窗口 / 退出），双击托盘唤起主窗口；**关闭窗口（含 Alt+F4）最小化到托盘**而非退出，真正退出走托盘「退出」；托盘不可用时关闭仍正常退出（防窗口被藏起无入口）。

### Fixed
- **输出/终端位置与整体布局**：主区域改为纵向布局（工具条在顶、编辑区居中、输出/终端沉底，IDEA 式）——此前 RunBar/编辑区/控制台三个 flex 子项横排，控制台成了编辑器旁边的竖条，整体凌乱。
- **对话「停止」没反应**：此前只取消了当前一次 AI 流，工具循环继续跑、状态不复位。现在「停止」停整条链：置停止标志逐步退出循环 + 取消 AI 流 + **终止正在跑的命令进程** + 关闭确认门，状态立即复位。
- **僵尸进程清理**：退出时（托盘退出/关闭退出）杀掉所有已登记子进程树（taskkill /T）并终止终端 ConPTY 会话；命令正常结束即解除进程登记，不再残留。

## [0.2.2] - 2026-10-12

### Fixed
- **大模块功能区串场**：此前图谱/日历与左侧面板叠加共存，切回「代码」时中心区仍停留在图谱，出现"图谱+代码树同屏、图谱冲掉代码功能区"。现恢复**每个大模块是独立功能区**：图谱/日历整块全幅替换工作区；点任何大模块都切换到该模块自己的功能区（同模块再点保留画布/小计内容视图），Ribbon 高亮与实际功能区严格一致。
- **Ctrl+P 检索打开文件不回编辑区**：图谱/日历全幅视图下经检索面板打开文件时，此前文件开了标签但视图留在原地，现统一回到编辑功能区。

## [0.2.1] - 2026-10-12

修复 0.2.0 实测问题：高频卡死、Maven 启动报错、笔记模块形态与布局细节。

### Fixed
- **卡死（主线程阻塞）**：28 个会启动子进程或做重 IO 的 Tauri 命令（Maven 依赖树、版本控制状态/差异、增量包、全文搜索、索引重建、打开工作区、文件导入等）全部移出主线程（async + spawn_blocking），UI 不再被 mvn / git / svn 子进程冻结。
- **Maven「%1 不是有效的 Win32 应用程序 (os error 193)」**：Windows 下可执行候选只接受 `.exe/.cmd/.bat`，Unix 版 `mvnw`/`bin/mvn` 脚本跳过并提示改用 `mvnw.cmd`；路径配错时给可读指引；`.cmd/.bat` 统一经 `cmd /C` 启动（工具链版本探测同修）。
- 右侧栏展开手柄在 RunBar 隐藏时的位置错位；文件树多选监听器泄漏。

### Changed
- **笔记模块改为独立清单**：不再显示项目树，与小计一致单独维护（按目录分组的笔记列表 + 筛选 + 新建/删除）。
- **模块顺序调整**：文档升至第二位（代码 / 文档 / 笔记 / 小计 / 标签 / 版本控制 / Maven / 图谱 / 日历）。
- **文件树视觉**：文件夹行加底色与加粗，与文件行区分。

## [0.2.0] - 2026-10-12

V2 配方系统（方案见《改造报告与方案V2.md》）+ V3 模块形态整修与问题修复（方案见《改造报告与方案V3.md》，决策 D1–D6 已确认）。

### Added
- **V2-P0 命令配方系统（Recipe）**：AI 首次执行的命令可沉淀为配方（`.easyide/recipes/`），指纹（构建文件 sha256 + 平台）命中即走本地快速通道直接 spawn（零 AI、零 token）；指纹失效自动检测并回落 AI 增量确认/重推理，配方升版时旧版归档 `history/` 可回滚。RunBar 动作带「直通 / 需确认」状态徽标；未信任配方首次直通前轻确认，可一键「执行并信任」。配方卡支持从本次 AI 命令候选中挑选、编辑命令/工作目录/就绪探针。
- **V2-P0 项目模型探测**：打开工作区自动识别项目类型（Maven 单/多模块、Gradle、npm/pnpm/yarn、pip/poetry/uv、Cargo、Go、CMake、混合仓库），输出模块清单、统一任务槽（run/build/test/deps/clean）与建议命令（配方卡预填）；语言统计与指纹文件清单（配方强指纹来源）。
- **V2-P0 配方安全**：危险命令硬拦截对快速通道同样生效；`.easyide/recipes/` 由配方命令层独占写入（`write_text_file` 拒绝写入，防 Agent 注入）；工具链环境变量按路径指纹缓存（消除重复子进程探测，保障快速通道 <50ms）。
- **版本控制 Git + SVN（V3-P2）**：统一 VCS 抽象（`src-tauri/src/vcs/`），打开项目自动探测 `.git`/`.svn`；SVN 支持状态/差异/日志/基线（增量包支持 svn revision 基线）；「版本控制」面板（原 Git 面板）Git/SVN 一键切换，状态栏显示分支或 `svn trunk@r1234`；文件树着色通用化；新增 SVN 工具链探测（设置 → 工具链）。
- **Maven 多模块（V3-P1）**：依赖树与一键 exclusion 支持模块选择（项目模型模块清单下拉）。

### Changed
- **模块重排（V3-P0，D1）**：Activity Bar 首模块改为「代码」（项目结构与代码开发，默认选中），「笔记」独立为第二模块（笔记树只含 .md/画布/图谱，新建笔记/画布归位其中）；代码树右键新增「新建文件」，「新建笔记」移出代码语境。
- **中心区视图矩阵（V3，D2/D4）**：图谱/日历/画布为全幅视图，左面板可叠加显示（点面板按钮不再被弹回编辑器）；RunBar 仅在代码/版本控制/Maven 开发语境显示；编辑器空状态按语境显示「打开或创建一个文件」（开发语境）或「选择或创建一篇笔记」（笔记语境）。
- **术语统一（V3，D5）**：「知识库」→「项目」，「未打开笔记」→「未打开文件」，「搜索笔记…」→「搜索项目…」，「库根目录」→「项目根目录」。
- **目录设置拆分（V3，D6）**：日记目录与小计目录拆为两个独立设置（daily_dir / jot_dir），小计不再落进日记目录。

### Fixed
- **Maven 启动失败 program not found（V3-P1）**：Maven 可执行解析改四级链（项目 mvnw > 设置路径 > 自动探测 > 检测指引报错），`.cmd/.bat` 经 `cmd /C` 启动，设置页「留空自动探测」承诺兑现；退出码判定不再静默吞错。
- 未跟踪文件 diff 空白（`git diff --no-index` 退出码 1 被当错误吞掉）、detached HEAD 显示 `HEAD@<短提交号>`、重命名路径含 ` -> ` 误切（V3-P2）。
- 图片文件从树中点开改为预览弹窗（原先走文本读取链路，二进制无法打开）；预览空态补文案（V3-P3）。
- 稳健性：Mutex 锁中毒不再 panic；危险命令识别收紧（命令词边界 + `cmd /C`、`bash -c` 内层递归解析，`mvn -Dformat=…` 等不再误伤）；配方保存时过危险校验；pom 备份/打包历史写入失败改为告警（V3-P3）。
- 清理死代码与 i18n 死键（Ribbon 未引用函数、文件树 graph 死分支、无引用文案）。

## [0.1.0] - 2026-10-08

由 EasyMD 0.2.0 分叉为独立产品 EasyIDE（轻量极速的 AI 代码 IDE）的首个版本，改造方案（P0-P5）见《改造报告与方案.md》。

### Added
- **多标签编辑器**（P0）：标签栏（脏标记/中键或 Ctrl+W 关闭/右键批量关闭）、Ctrl+Tab 切换、按工作区恢复会话。
- **多语言代码编辑**（P0）：CodeMirror 6 语言包按需动态加载（Java/XML/JSON/JS/TS/Python/C++/HTML/CSS/SQL/YAML/TOML/Properties/Shell），行号、折叠、括号匹配、等宽字体；文件树按类型显示图标、编译产物目录置灰、项目点文件可见。
- **代码索引与四级检索**（P1）：Rust 符号索引引擎（类/接口/枚举/方法/函数/字段，mtime 增量 + JSON 缓存 + watcher 实时更新）；Ctrl+P 文件 / Ctrl+N 类 / Ctrl+Shift+Alt+N 符号 / Ctrl+Shift+F 并行全文搜索；右侧大纲面板支持代码符号跳转。
- **项目文档中心**（P2，移植 idea-md-assistant）：项目内 MD 一键扫描（28 排除目录 + 31 AI 工具目录识别）、五级分类链（手动映射/知识库目录/AI 目录/AI 缓存 mtime 失效/规则打分）、四分组模式、归档物理移动、ZIP 打包、AGENTS.md 标记块合并生成、AI 批量分类与 5 个快捷操作；知识库项目维度化（.easyide/）。
- **AI Agent 中枢**（P3）：流式对话（OpenAI 兼容/Anthropic 双协议 SSE，可取消）、工具调用循环（read_file/list_dir/search_text/search_symbols/run_command，最多 8 轮）、命令确认门 + 危险命令双层拦截、底部控制台（输出流/状态/进程树终止）、会话持久化、AGENTS.md 自动注入系统提示词、工具链配置中心（JDK/Maven/Node/Git 自动探测与环境注入）、RunBar 七个预设场景（运行/日志/构建/依赖/修复/解读/测试）。
- **Git 集成**（P4）：文件树修改/新增/删除着色、分支与变更面板、unified diff 视图、AI 生成提交信息。
- **Maven 工具箱**（P4）：依赖树解析（多模块）、版本冲突高亮与过滤、一键 exclusion 写入 pom.xml（自动备份）、AI 分析冲突。
- **增量包**（P4）：git tag/commit 基线、classes/war/fat-jar 三模板、内部类收集、多模块映射、打包历史。
- 设置新增「工具链」「快捷键」页签；快捷键重排（Ctrl+N 类搜索、Ctrl+Alt+N 新建笔记）。
- 画布/图谱/日历与各语言包懒加载分包，主包体积较 easymd 下降。

### Fixed
- 修复 SettingsDialog 面板 transition 多子元素模板错误（easymd 带来的潜伏问题，dev 模式阻塞）。

## [EasyMD 0.2.0] - 2026-10-07

### Added
- 设置新增「关于」页签：集中展示产品名、版本号（运行时读取）、作者、联系邮箱（一键复制）、GitHub 源码仓库链接与开源许可证，应用内即可联系作者或跳转仓库。

### Changed
- 统一发布人身份：package.json 作者、Git 提交身份统一为 clb <lamthebest@foxmail.com>。

### Fixed

## [EasyMD 0.1.0] - 2026-10-06

首个发布版本：本地优先的 Markdown 知识库桌面应用（Tauri 2 + Vue 3 + Rust）。

### Added
- Markdown 编辑器（源码/预览双模式）与 .canvas 画布。
- 双向链接、反链面板、关系图谱、文件管理、标签、全文搜索、日记、小计时间线、日历看板。
- AI 辅助与 MCP 云同步（外部服务）。

