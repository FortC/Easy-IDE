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
| P2 | 项目文档中心（MD 扫描/分类/知识库，移植 idea-md-assistant） | ⏳ |
| P3 | AI Agent 中枢（流式/多轮/工具调用）+ 工具链配置（JDK/Maven）+ 控制台 + 预设场景 | ⏳ |
| P4 | Git 集成、Maven 依赖树/冲突（Maven Helper 等价）、增量包（war/fat-jar 模板）、交互终端 | ⏳ |
| P5 | 主题/快捷键设置页、i18n 补全、性能冲刺、1.0.0 发布 | ⏳ |

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

## License

Apache-2.0
