# Changelog

本项目所有显著变更均记录于此，格式遵循 [Keep a Changelog](https://keepachangelog.com/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

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

## [0.2.0] - 2026-10-07

### Added
- 设置新增「关于」页签：集中展示产品名、版本号（运行时读取）、作者、联系邮箱（一键复制）、GitHub 源码仓库链接与开源许可证，应用内即可联系作者或跳转仓库。

### Changed
- 统一发布人身份：package.json 作者、Git 提交身份统一为 clb <lamthebest@foxmail.com>。

### Fixed

## [0.1.0] - 2026-10-06

首个发布版本：本地优先的 Markdown 知识库桌面应用（Tauri 2 + Vue 3 + Rust）。

### Added
- Markdown 编辑器（源码/预览双模式）与 .canvas 画布。
- 双向链接、反链面板、关系图谱、文件管理、标签、全文搜索、日记、小计时间线、日历看板。
- AI 辅助与 MCP 云同步（外部服务）。

