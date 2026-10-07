# EasyIDE 项目规则（AI 工具必读）

本项目是轻量极速的 AI 代码 IDE（Tauri 2 + Vue 3 + TypeScript），由 EasyMD 0.2.0 分叉而来。UI 沿用 Obsidian 风格设计令牌，不做自由发挥。任何代码/样式改动必须遵守以下规则。

## 设计令牌（唯一色值来源：`src/assets/theme.css`）

- 禁止在组件里硬编码 hex 色值，一律使用 CSS 变量。
- 暗色：`--background-primary:#1e1e1e` / `--background-secondary:#161616` / 边框 `#333` / 正文 `#dcddde` / 弱字 `#999` / 强调紫 `#8b6cef`。
- 亮色：`#ffffff` / `#f6f6f6` / 边框 `#dbdbdc` / 正文 `#222` / 弱字 `#666` / 强调紫 `#8b6cef`。
- 字体：UI 13px、正文 16px，系统栈 `Inter → Segoe UI Variable → Segoe UI → 微软雅黑`；等宽 `JetBrains Mono → Cascadia Code → Consolas`。
- 圆角：4 / 8 / 12px 三档；阴影只用 `--shadow-l1/l2`。

## 布局（IDE 形态，沿 Obsidian 视觉语言）

- 结构：左侧 44px 竖排 Activity Bar → 侧边面板（资源管理器/搜索/文档中心，宽 250px 可拖）→ 主编辑区（多标签）→ 右侧 AI 助手停靠（宽 300px 可拖）→ 底部控制台/状态栏（24px）。
- 无框窗口：顶栏 40px 兼作拖拽区，右侧自绘最小化/最大化/关闭按钮。
- 弹窗 = Obsidian 风格模态：居中、圆角 12px、背景 `--background-secondary`。
- 命令面板/检索面板 = 居中偏上搜索弹窗（Ctrl+P 文件 / Ctrl+N 类 / Ctrl+Shift+F 全文）。

## 禁止项

- ❌ 禁止 Emoji 当图标；图标一律用内联 Lucide SVG（见 `src/components/common/Icon.vue` + `iconPaths.ts`）。
- ❌ 禁止蓝紫渐变、玻璃拟态、营销页式 Hero、"三卡片"模板化布局。
- ❌ 禁止 ease-in-out 默认缓动；动效用 `--anim-fast/--anim-medium`（cubic-bezier(0.33,0.66,0.5,1)），只做克制微反馈（hover/展开），不做入场炫技。
- ❌ 界面文案使用简体中文、具体直白，不写"赋能/极致/智能"类套话。
- ❌ 禁止引入 LSP 服务器、Electron、Monaco 等重依赖（性能红线，见改造方案 3.1）。

## 架构红线（业务）

- 磁盘文件是唯一真相源；索引只是缓存（存 app 配置目录，可随时重建），永远不要用缓存反写文件。
- 工作区内私有状态（文档分类、AI 会话等）放 `<项目根>/.easyide/`；软件全局私有数据放 Tauri app 配置目录。
- 所有文件 IO 与命令执行走 Rust 命令（src-tauri），前端不直接碰文件系统/进程。
- AI 执行命令必须经过确认门（默认每次确认 + 危险模式硬拦截），无用户批准不得落盘写文件、不得执行 shell 命令。
- 禁止账号体系、云 SDK；AI 走用户自配的 OpenAI 兼容 / Anthropic 端点，Key 只存本地。
- 性能预算是硬指标：冷启动 <1.2s、打开文件 <80ms、10k 文件索引 <3s；新增功能不得破坏（重模块必须懒加载）。
- 保留自 EasyMD 的笔记/小计/日历/画布/图谱能力，作为「项目文档中心」的一部分演进，不得删除。

## 技术栈

- 前端：Vue 3 `<script setup lang="ts">` + Pinia + CodeMirror 6（多语言按需加载）+ markdown-it + force-graph。
- 后端：Rust（tauri 2 命令层 + index/ 索引引擎 + agent/ AI 中枢 + toolchain/ 工具链）。
- 验证：改前端必须过 `npm run build`；改 Rust 必须过 `cargo check` / `cargo test`。
- 完整路线见《改造报告与方案.md》（P0-P5 分阶段，已确认定稿）。
