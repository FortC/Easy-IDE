# EasyIDE 项目规则（AI 工具必读）

本项目是轻量极速的 AI 代码 IDE（Tauri 2 + Vue 3 + TypeScript），由 EasyMD 0.2.0 分叉而来。UI 沿用 Obsidian 风格设计令牌，不做自由发挥。任何代码/样式改动必须遵守以下规则。

## 设计令牌（唯一色值来源：`src/assets/theme.css`）

- 禁止在组件里硬编码 hex 色值，一律使用 CSS 变量。
- 暗色：`--background-primary:#1e1e1e` / `--background-secondary:#161616` / 边框 `#333` / 正文 `#dcddde` / 弱字 `#999` / 强调紫 `#8b6cef`。
- 亮色：`#ffffff` / `#f6f6f6` / 边框 `#dbdbdc` / 正文 `#222` / 弱字 `#666` / 强调紫 `#8b6cef`。
- 字体：UI 13px、正文 16px，系统栈 `Inter → Segoe UI Variable → Segoe UI → 微软雅黑`；等宽 `JetBrains Mono → Cascadia Code → Consolas`。
- 圆角：4 / 8 / 12px 三档；阴影只用 `--shadow-l1/l2`。

## 布局（IDE 形态，沿 Obsidian 视觉语言）

- 结构：**48px 纯图标 Activity Bar**（`components/common/ActivityBar.vue`，8 个大模块入口，hover 时图标淡出、模块名淡入）→ 侧边面板（宽 268px 可拖）→ 主编辑区（多标签）→ 右侧停靠栏（320px 可拖）→ 底部控制台（输出/终端/问题）→ 状态栏（24px）。
- **导航只放「主区域的大模块」**，语义单一：项目结构 / 文档中心 / 版本控制 / Maven / 笔记 / 标签 / 图谱 / 日历。前 6 个切左栏面板；图谱、日历是主区全幅视图（左右栏保持可见，不再塞进右栏 tab）；画布、小计是文件类型视图，走主区。
- 无框窗口：顶栏 36px 兼作拖拽区，右侧自绘最小化/最大化/关闭按钮。
- 弹窗 = Obsidian 风格模态：居中、圆角 12px、背景 `--background-secondary`。
- 命令面板/检索面板 = 居中偏上搜索弹窗（Ctrl+P 文件 / Ctrl+N 类 / 全文搜索双击 Shift 唤起，Ctrl+Shift+F 为可改绑的备用键）。
- 栏宽偏好存 localStorage（`easyide-panel-widths`），拖拽调宽时全局禁动画保证 1:1 跟手。

## 语境模型（双语境产品的核心抽象）

EasyIDE 同时服务「代码开发」与「笔记/小计记录」两类任务，UI 的一切显隐由**语境**驱动：

- 判定：`ui.context = f(左栏 activePanel, 当前打开文件类型)`，用 `isMarkdown(path)` 双因子判定，见 `stores/ui.ts` 的 `context` getter。
- 代码语境：RunBar 常驻；右栏 = AI 助手 / 代码大纲 / 引用查找 / AI 记忆；底部含「问题」页签。
- 笔记语境：RunBar 隐藏；右栏 = AI 助手 / 大纲 / 反链 / 属性；底部无「问题」页签。
- **反链（md 双向链接）与属性（frontmatter）是笔记语境专属功能，严禁出现在代码开发语境**。
- 右栏 tab 集由 `ui.rightTabs` getter 派生；语境切换时调 `reconcileRightTab()` 校正（不在新集合内则回落 AI，**不重置 AI 会话**）。
- AI 快捷动作集同理：`stores/actions.ts` 的 `actionsOf(context)`，代码语境 7 个（运行/调试/构建/依赖/修复/解读/测试），笔记语境 5 个（总结/续写/生成标签/转小计/生成大纲）。RunBar 与 AgentPanel 的 chips **共用这份定义**，禁止各写一套。

## 动效七条红线

动效只做「让状态变化可被感知」，不做入场炫技。令牌在 `theme.css`，动效样式在 `anim.css`。

1. **只动 `opacity` / `transform`** —— 走合成器，不触发 reflow。
2. **禁止动 `width` / `height` / `top` / `left`** —— 大列表直接掉帧。
3. **离场时长 ≤ 入场时长** —— 保证「切得快」；离场用 `--anim-leave`(80ms)，入场用 `--anim-enter`(180ms)。
4. **单个动画 ≤ 300ms** —— 超过是「卡」不是「丝滑」。
5. **拖拽/滚动中禁一切过渡** —— 见 `body.is-col-resizing *`，保证 1:1 跟手。
6. **禁止 JS 动画库** —— 保持零 GSAP/anime 依赖。
7. **AI 流式文字不做逐字动画** —— 避免持续重排，长回答必掉帧。

- 业务代码只用语义令牌：`--anim-hover` / `--anim-press` / `--anim-enter` / `--anim-leave`；底层 `--anim-ease-out/in/spring` 与 `--dur-instant/fast/base/slow`。
- **高频交互必须有 `:active` 按压反馈**（用 `transform: scale(0.96)`，立即生效）；`@media (hover:none)` 下禁用。
- ⚠️ **scoped 样式组件必须在组件内单独写按压反馈** —— `anim.css` 的全局选择器不命中 scoped 组件（属性后缀隔离）。
- 设置项提供动效强度三档（`data-motion="full|reduced|off"`），与系统 `prefers-reduced-motion` 双保险。

## AI 集成约定

- **单一入口**：所有 AI 能力都导向右侧 AI 面板，禁止再弹出独立 AI 对话框。
- **选区上下文**：编辑器选中代码自动挂载到 `agentStore.contextSelection`，AI 提问时以结构化上下文注入（AI 无需再 `read_file` 找回来）。代码模式下 **Ctrl+I 唤起 AI 面板**（md 模式下 Ctrl+I 是斜体，勿冲突）。
- 上下文注入一次即清空，避免同一选区反复污染后续提问。
- 工具调用必须可视化：语义图标 + 中文标签 + 步骤序号 + 完成/拒绝徽标；执行中用三点脉冲，**不要用旋转 spinner**。
- 命令确认门必须内联在会话流中（含命令高亮 + diff 预览），不得跳出上下文。

## 禁止项

- ❌ 禁止 Emoji 当图标；图标一律用内联 Lucide SVG（见 `src/components/common/Icon.vue` + `iconPaths.ts`）。
- ❌ 禁止蓝紫渐变、玻璃拟态、营销页式 Hero、"三卡片"模板化布局。
- ❌ 禁止 ease-in-out 默认缓动与硬编码毫秒数；动效用语义令牌（见动效七条红线）。
- ❌ 界面文案使用简体中文、具体直白，不写"赋能/极致/智能"类套话。
- ❌ 禁止引入 LSP 服务器、Electron、Monaco 等重依赖（性能红线，见改造方案 3.1）。
- ❌ **禁止把 markdown-it / yaml 从首屏主包引入**：渲染器已拆到 `lib/markdown/md-render.ts`（懒载入），纯文本工具留在 `renderer.ts`；新增重依赖必须走动态 `import()`。

## 架构红线（业务）

- 磁盘文件是唯一真相源；索引只是缓存（存 app 配置目录，可随时重建），永远不要用缓存反写文件。
- 工作区内私有状态（文档分类、AI 会话等）放 `<项目根>/.easyide/`；软件全局私有数据放 Tauri app 配置目录。
- 所有文件 IO 与命令执行走 Rust 命令（src-tauri），前端不直接碰文件系统/进程。
- AI 执行命令必须经过确认门（默认每次确认 + 危险模式硬拦截），无用户批准不得落盘写文件、不得执行 shell 命令。
- **命令配方（`.easyide/recipes/`）由 recipe 命令层独占写入**：`write_text_file` / Agent `write_file` 一律不得写该目录（防注入）；配方快速通道执行同样过危险命令硬拦截。
- **配方失效宁可重问用户，不得盲执行过期命令**：指纹（构建文件 sha256 + 平台）失效即回落 AI 增量确认或重推理，成功后升版沉淀。
- 禁止账号体系、云 SDK；AI 走用户自配的 OpenAI 兼容 / Anthropic 端点，Key 只存本地。
- 性能预算是硬指标：冷启动 <1.2s、打开文件 <80ms、10k 文件索引 <3s；新增功能不得破坏（重模块必须懒加载）。
  - **首屏主包 JS ≤ 500KB**（当前 243.0 KB）、**首屏 CSS ≤ 80KB**（当前 71.10 KB）。
  - **包体由 `npm run verify` 自动判定**（`scripts/check-bundle.mjs` + `docs/performance-budget.json`），新增首屏重依赖会被门禁挡下。
  - 首屏只允许加载：编辑区 + 文件树 + AI 面板 + 大纲 + 底部控制台 + vendor 三件套。
  - 任何进入首屏路径的重依赖必须走动态 `import()` 或 `defineAsyncComponent`。
- 保留自 EasyMD 的笔记/小计/日历/画布/图谱能力，作为「项目文档中心」的一部分演进，不得删除（但可调整入口层级）。

## 技术栈

- 前端：Vue 3 `<script setup lang="ts">` + Pinia + CodeMirror 6（多语言按需加载）+ markdown-it + force-graph。
- 后端：Rust（tauri 2 命令层 + index/ 索引引擎 + agent/ AI 中枢 + toolchain/ 工具链）。
- 验证：改前端必须过 `npm run build`；改 Rust 必须过 `cargo check` / `cargo test`。
- 完整路线见《改造报告与方案.md》（P0-P5 分阶段，已确认定稿）。
