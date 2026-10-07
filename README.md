# EasyIDE

轻量、极速、AI 原生的代码 IDE。**给足配置（JDK / Maven / 大模型 Key），其余的拉依赖、构建、运行、Debug 日志、问题诊断都交给 AI 执行。**

由 [EasyMD](../easymd)（Markdown 知识库编辑器）0.2.0 分叉而来，两条产品线独立维护：EasyMD 继续做笔记，EasyIDE 做开发。

## 核心理念

- **极致快速**：Tauri 2 壳 + CodeMirror 6 内核，冷启动 <1.2s，安装包 <15MB，不引入 LSP 服务器等重资产。
- **基本功能健全**：多标签编辑、多语言语法高亮、文件/类名/方法名/全文四级检索、控制台、Git 感知。
- **AI 优先执行**：内置 AI Agent（流式对话 + 工具调用），预设「运行 / Debug 日志 / 构建 / 拉依赖 / 修复报错」等场景，命令执行带确认门。
- **保留笔记基因**：EasyMD 的文档中心、双链、小计、日历全部保留，按项目维度组织（`.easyide/`），并复刻 idea-md-assistant 的项目 MD 扫描/分类/归档能力。
- **本地优先**：磁盘文件是唯一真相源，AI Key 只存本地，无账号无云依赖。

## 路线图（详见《改造报告与方案.md》，已确认）

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 基座改造：工作区、多标签编辑器、多语言高亮、文件树代码化、代码分割 | ✅ 已完成 |
| P1 | 符号索引 + 四级快速检索（Ctrl+P / Ctrl+N / Ctrl+Shift+Alt+N / Ctrl+Shift+F）+ 代码大纲 | ✅ 已完成 |
| P2 | 项目文档中心（MD 扫描/五级分类链/归档/ZIP/AGENTS.md/AI 分类与快捷操作，知识库项目维度 .easyide/） | ✅ 已完成 |
| P3 | AI Agent 中枢（流式/多轮/工具调用循环 + 命令确认门）+ 工具链配置（JDK/Maven/Node/Git 自动探测）+ 控制台 + 7 个预设场景（运行/日志/构建/依赖/修复/解读/测试） | ✅ 已完成 |
| P4 | Git 集成（状态着色/分支面板/diff/AI 提交）、Maven 依赖树/冲突/一键排除（Maven Helper 等价）、增量包（tag/commit 基线 + war/fat-jar/classes 模板 + 历史） | ✅ 已完成 |
| P5 | 快捷键速查页、CHANGELOG、发布构建（NSIS 安装包 2.84MB） | ✅ 已完成 |

## 技术栈

- 前端：Vue 3 + TypeScript + Pinia + Vite 6 + CodeMirror 6（语言包按需加载）
- 桌面：Tauri 2（Windows 优先）
- 后端：Rust（索引引擎 / AI Agent / 工具链探测 / 命令执行）

## 开发

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 开发模式运行
npm run build        # 前端类型检查 + 构建
cargo check          # Rust 侧检查（在 src-tauri/ 下）
```

## 数据存放

- 项目级状态：`<项目根>/.easyide/`（文档分类、AI 会话、运行预设）
- 用户设置 / 索引缓存：`%APPDATA%/com.easyide.app/`

## 性能实测（release 构建，10,000 文件合成基准）

| 指标 | 预算 | 实测 |
|---|---|---|
| 10k 文件全量索引 | < 3s | **240 ms** |
| 索引缓存写入 / 加载 | — | 11.5 ms / 8.8 ms |
| 符号搜索 | < 30ms | **3.4 ms** |
| 全文搜索（10k 文件并行） | < 300ms | **37 ms** |
| 安装包 | < 15MB | **2.84 MB** |

（基准复跑：`cargo test --release perf_index_10k -- --ignored --nocapture`；前端就绪耗时见 设置 → 关于）

## License

Apache-2.0
