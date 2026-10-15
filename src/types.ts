// 与 Rust 端 serde 模型一一对应（字段名保持 snake_case）
export interface Heading {
  level: number;
  text: string;
  line: number;
}

export interface NoteLink {
  raw: string;
  target: string;
  subpath: string | null;
  alias: string | null;
  embed: boolean;
  line: number;
}

export interface BlockId {
  id: string;
  line: number;
}

export interface NoteIndex {
  path: string;
  title: string;
  aliases: string[];
  tags: string[];
  headings: Heading[];
  links: NoteLink[];
  block_ids: BlockId[];
  mtime: number;
  size: number;
}

export interface Backlink {
  source: string;
  source_title: string;
  link: NoteLink;
}

export interface TagCount {
  tag: string;
  count: number;
}

export interface ResolveResult {
  path: string | null;
  ambiguous: boolean;
}

export type EntryKind = "dir" | "md" | "canvas" | "graph" | "image" | "other";

export interface FsEntry {
  name: string;
  path: string;
  is_dir: boolean;
  kind: EntryKind;
}

export interface VaultEntry {
  path: string;
  name: string;
  last_opened: string;
}

export interface OpenVaultResult {
  root: string;
  notes: NoteIndex[];
}

export interface McpToolMap {
  list?: string | null;
  download?: string | null;
  upload?: string | null;
  delete?: string | null;
  mkdir?: string | null;
}

export interface McpServerConfig {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  enabled: boolean;
  tool_map: McpToolMap;
}

export interface SnippetState {
  name: string;
  enabled: boolean;
}

export interface AppSettings {
  theme: string;
  attachments_dir: string;
  templates_dir: string;
  daily_dir: string;
  /** 小计目录（V3-D6 与日记目录拆分） */
  jot_dir: string;
  daily_format: string;
  daily_template?: string | null;
  daily_tag: string;
  last_vault: string | null;
  mcp_configs: McpServerConfig[];
  active_mcp: string | null;
  sync_interval_minutes: number;
  css_snippets: SnippetState[];
  ai_provider: string;
  ai_base_url: string;
  ai_api_key: string;
  ai_model: string;
  language: string;
  font_scale: string;
  /** 动效强度：full 完整 / reduced 精简（仅微反馈）/ off 关闭；与系统 prefers-reduced-motion 双保险 */
  motion_level: MotionLevel;
  /** 代码配色主题："auto" 跟随界面，其余见 lib/editorThemes.ts 的 CODE_THEMES */
  code_theme: string;
  log_template: string;
  log_templates_dir: string;
  log_template_name: string;
  jdk_path: string;
  maven_path: string;
  node_path: string;
  git_path: string;
  svn_path: string;
  keybindings: Record<string, string>;
  /** B7-B 档：启用本地模型端点（Ollama / llama.cpp 等 OpenAI 兼容服务） */
  local_llm_enabled: boolean;
  /** B7-B 档：本地端点 baseURL，如 http://localhost:11434/v1 */
  local_llm_base: string;
  /** B7-B 档：模型名，如 qwen2.5-coder:7b */
  local_llm_model: string;
}

export const defaultSettings = (): AppSettings => ({
  theme: "dark",
  attachments_dir: "assets",
  templates_dir: "templates",
  daily_dir: "daily",
  jot_dir: "jots",
  daily_format: "YYYY-MM-DD",
  daily_template: null,
  daily_tag: "",
  last_vault: null,
  mcp_configs: [],
  active_mcp: null,
  sync_interval_minutes: 0,
  css_snippets: [],
  ai_provider: "openai",
  ai_base_url: "https://api.openai.com/v1",
  ai_api_key: "",
  ai_model: "gpt-4o-mini",
  language: "zh",
  font_scale: "sm",
  motion_level: "full",
  code_theme: "auto",
  log_template: "",
  log_templates_dir: "log-templates",
  log_template_name: "",
  jdk_path: "",
  maven_path: "",
  node_path: "",
  git_path: "",
  svn_path: "",
  keybindings: {},
  // 本地模型默认关闭：用户没装 Ollama 时开着只会让请求全部失败
  local_llm_enabled: false,
  local_llm_base: "http://localhost:11434/v1",
  local_llm_model: "",
});

/** B7-B 档：本地模型端点探测结果（后端 local_llm::LocalLlmProbe） */
export interface LocalLlmProbe {
  reachable: boolean;
  models: string[];
  elapsed_ms: number;
  error: string;
}

/** vault-changed 事件载荷 */
export interface VaultChangedPayload {
  updated: NoteIndex[];
  removed: string[];
  canvas_changed: string[];
  graph_changed: string[];
  /** 变化的代码/文本文件（P1：代码索引增量） */
  code_changed: string[];
}

/** 代码符号（P1 代码索引） */
export interface CodeSymbol {
  /** class | interface | enum | record | method | function | field */
  kind: string;
  name: string;
  /** 1 基行号 */
  line: number;
  container: string | null;
}

export interface SymbolHit extends CodeSymbol {
  path: string;
}

export interface CodeFileHit {
  path: string;
  name: string;
}

export interface TextHit {
  path: string;
  /** 1 基行号 */
  line_no: number;
  text: string;
}

/** 文档中心（P2，移植 idea-md-assistant） */
export interface DocEntry {
  path: string;
  name: string;
  mtime: number;
  size: number;
  from_ai_dir: boolean;
  date_key: string;
  category: string | null;
}

export interface DocsStateView {
  knowledge_dirs: [string, string][];
  ai_tool_dirs: string;
  show_ai_tool_docs: boolean;
  default_group: string;
  agents_target: string;
  rule_extra: string;
  ignored_count: number;
}

export interface DocsScanResult {
  entries: DocEntry[];
  categories: string[];
  state: DocsStateView;
}

export interface DocsSettingsPatch {
  knowledge_dirs?: [string, string][];
  ai_tool_dirs?: string;
  show_ai_tool_docs?: boolean;
  default_group?: string;
  agents_target?: string;
  rule_extra?: string;
  categories?: string[];
}

export interface DocsClassifyProgress {
  done: number;
  total: number;
}

/** .graph 图谱文件（JSON）模型 */
export interface GraphNode {
  id: string;
  label: string;
  /** 绑定的笔记相对路径（双击可打开） */
  note?: string;
  x?: number;
  y?: number;
}

export interface GraphEdge {
  source: string;
  target: string;
  label?: string;
}

export interface GraphDoc {
  version: 1;
  mode: "manual";
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export type EditMode = "source" | "preview" | "split";
export type MainView = "editor" | "graph" | "canvas" | "calendar" | "jot";
/** 左侧模块（V3 重排：代码第一、笔记第二、版本控制收编 Git+SVN） */
export type LeftTab = "code" | "notes" | "jots" | "tags" | "docs" | "vcs" | "maven";
/**
 * 右栏 tab（随语境自适应，见 stores/ui.ts 的 rightTabs getter）
 * - dev 语境：ai / outline / refs（符号级引用查找）/ memory（AI 记忆）
 * - note 语境：ai / outline / backlinks（md 双向链接）/ props（frontmatter）
 * 注：图谱与日历是左侧大模块（活动栏入口 → 主区全幅视图，见 MainView），不进右栏。
 */
export type RightTab =
  | "outline"
  | "backlinks"
  | "props"
  | "ai"
  | "refs"
  | "memory";
/** 语境：代码开发 vs 笔记小计（双语境产品的核心抽象） */
export type WorkContext = "dev" | "note";
/** 动效强度（与系统 prefers-reduced-motion 双保险） */
export type MotionLevel = "full" | "reduced" | "off";

/** AI Agent（P3） */
export interface AgentMessage {
  role: "user" | "assistant";
  content: string;
}

export interface SessionMeta {
  id: string;
  title: string;
  updated: string;
}

export interface SessionDoc {
  id: string;
  title: string;
  updated: string;
  messages: AgentMessage[];
}

/** 版本控制（V3：Git + SVN 统一抽象） */
export interface VcsFileStatus {
  path: string;
  /** 统一状态字母：M 修改 / A 新增 / D 删除 / R 重命名 / ? 未跟踪 */
  status: string;
}

export interface VcsStatus {
  /** "git" | "svn" | "none" */
  kind: string;
  is_repo: boolean;
  /** git 分支名（detached 时为 HEAD@<短提交号>）；svn 为 URL 尾段（trunk/branches/x） */
  branch: string;
  /** git 短提交号；svn 为 r<版本号> */
  revision: string;
  /** 已统一重映射为「工作区相对路径」，可直接和文件树对上 */
  files: VcsFileStatus[];
  /** 仓库根相对工作区根的路径；null/undefined = 仓库根就是工作区根 */
  repo_root?: string | null;
  /** 仓库根在工作区根的子目录里 —— 需提示用户「你打开的是子目录」 */
  nested?: boolean;
}

export interface CommitInfo {
  hash: string;
  subject: string;
  date: string;
}

/** 文件树右键「构建命令」探测结果：右键目标沿目录链向上找到的构建模块 */
export interface BuildTarget {
  /** "maven" | "gradle" | "npm" */
  kind: string;
  /** 模块目录（工作区相对路径，"" = 工作区根） */
  dir: string;
  /** 构建文件相对路径 */
  file: string;
  /** 模块显示名（目录名；根模块为空串） */
  name: string;
  /** npm scripts 名（仅 npm） */
  scripts: string[];
  /** npm 包管理器 npm | pnpm | yarn（仅 npm） */
  pm: string;
}

/**
 * 保存的动作按钮（B5.2）—— 与后端 `.easyide/actions.json` 的线上格式。
 * 注意是 snake_case：Rust 侧 serde 默认不做 camelCase 转换。
 * 前端模型见 `stores/savedActions.ts` 的 SavedAction（camelCase）。
 */
export interface SavedActionWire {
  id: string;
  label: string;
  /** "command" | "prompt" */
  kind: string;
  command: string | null;
  prompt: string | null;
  /** 长驻进程：点按钮不等待退出，需显式停止 */
  background: boolean;
  ready_pattern: string | null;
  icon: string;
  action_key: string | null;
  order: number;
}

/** Maven 依赖树（P4） */
export interface DepNode {
  gav: string;
  group: string;
  artifact: string;
  version: string;
  scope: string;
  depth: number;
  conflict_with: string | null;
  children: DepNode[];
}

export interface ModuleTree {
  module: string;
  root: DepNode;
}

/** 增量包（P4） */
export interface PkgResult {
  zip_path: string;
  entries: string[];
  skipped: string[];
}

export interface PkgHistoryItem {
  time: string;
  baseline: string;
  template: string;
  zip: string;
  count: number;
}

export interface PkgBaselines {
  tags: string[];
  commits: CommitInfo[];
}

export interface ToolInfo {
  kind: string;
  found: boolean;
  path: string;
  version: string;
  error: string;
}

// ==================== V2-P0：命令配方与项目模型 ====================

/** 配方指纹与约束 */
export interface RecipePreconditions {
  platform: string;
  toolchain: Record<string, string>;
  fingerprint: Record<string, string>;
}

export interface RecipeExpect {
  success_exit_codes: number[];
  ready_pattern: string;
  ready_timeout_ms: number;
  port: number;
}

export interface RecipeMeta {
  version: number;
  status: string;
  source: string;
  model: string;
  conversation_id: string;
  prompt: string;
  created_at: string;
  created_by: string;
  last_used_at: string;
  usage_count: number;
  last_exit_code: number | null;
  avg_duration_ms: number;
  trusted: boolean;
}

/** 命令配方（.easyide/recipes/<id>.json） */
export interface Recipe {
  schema: number;
  action: string;
  variant: string;
  shell: string;
  cwd: string;
  command: string;
  env: Record<string, string>;
  preconditions: RecipePreconditions;
  expect: RecipeExpect;
  meta: RecipeMeta;
}

/** 配方状态摘要（RunBar 徽标） */
export interface RecipeStatus {
  id: string;
  action: string;
  variant: string;
  command: string;
  cwd: string;
  valid: boolean;
  changed: string[];
  usage_count: number;
  avg_duration_ms: number;
  trusted: boolean;
  status: string;
}

/** 保存配方的前端输入（指纹/版本由后端补齐） */
export interface RecipeInput {
  action: string;
  variant?: string;
  command: string;
  cwd?: string;
  env?: Record<string, string>;
  ready_pattern?: string;
  port?: number;
  source?: "ai" | "user";
  model?: string;
  conversation_id?: string;
  prompt?: string;
  trusted?: boolean;
}

/** 快速通道执行结果 */
export interface RecipeRunResponse {
  ok: boolean;
  reason: string;
  changed: string[];
  cmd_id: number | null;
  command: string;
}

/** 项目模块 */
export interface ProjectModule {
  name: string;
  path: string;
  kind: string;
  entry: string;
  port: number;
}

/** B4 离线模块树节点（后端 maven::pom::ModuleNode，serde 直出 snake_case） */
export interface PomModuleNode {
  /** 相对工作区根的路径，根为 "." */
  rel_path: string;
  group_id: string;
  artifact_id: string;
  version: string;
  packaging: string;
  /** 父模块 rel_path，null = 顶层聚合 pom */
  parent: string | null;
  /** 0 = 顶层聚合 pom */
  depth: number;
  module_count: number;
  dep_count: number;
  managed_count: number;
  /** 含 spring-boot-maven-plugin，可直接 spring-boot:run */
  is_boot_app: boolean;
  /** 非空说明该 pom 解析失败（其它字段可能不完整） */
  error: string;
}

/** B4 离线模块依赖（后端 maven::pom::PomDep） */
export interface PomDep {
  group_id: string;
  artifact_id: string;
  version: string;
  scope: string;
  optional: boolean;
  exclusions: string[];
}

/** 统一任务槽 */
export interface ProjectTask {
  action: string;
  module: string;
  kind: string;
  suggest: string;
}

/** 项目模型（打开工作区探测） */
export interface ProjectModel {
  type: string;
  languages: string[];
  modules: ProjectModule[];
  tasks: ProjectTask[];
  fingerprint_files: string[];
  suggests: Record<string, string>;
  detected_at: string;
}

/** AI 执行过的命令记录（配方捕获候选） */
export interface CommandLogEntry {
  command: string;
  code: number | null;
  tail: string;
}
