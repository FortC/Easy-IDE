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
  log_template: string;
  log_templates_dir: string;
  log_template_name: string;
  jdk_path: string;
  maven_path: string;
  node_path: string;
  git_path: string;
  keybindings: Record<string, string>;
}

export const defaultSettings = (): AppSettings => ({
  theme: "dark",
  attachments_dir: "assets",
  templates_dir: "templates",
  daily_dir: "daily",
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
  log_template: "",
  log_templates_dir: "log-templates",
  log_template_name: "",
  jdk_path: "",
  maven_path: "",
  node_path: "",
  git_path: "",
  keybindings: {},
});

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
export type LeftTab = "notes" | "jots" | "tags" | "docs" | "git" | "maven";
export type RightTab = "outline" | "backlinks" | "props" | "ai";

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

/** Git 集成（P4） */
export interface GitFileStatus {
  path: string;
  /** M / A / D / R / ? */
  status: string;
}

export interface GitStatus {
  is_repo: boolean;
  branch: string;
  files: GitFileStatus[];
}

export interface CommitInfo {
  hash: string;
  subject: string;
  date: string;
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
