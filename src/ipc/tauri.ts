// Tauri IPC 类型化封装：前端唯一的后端入口
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentMessage,
  AppSettings,
  Backlink,
  SessionDoc,
  SessionMeta,
  ToolInfo,
  CodeFileHit,
  CodeSymbol,
  DocEntry,
  DocsClassifyProgress,
  DocsScanResult,
  DocsSettingsPatch,
  FsEntry,
  McpServerConfig,
  NoteIndex,
  OpenVaultResult,
  ResolveResult,
  SymbolHit,
  TagCount,
  TextHit,
  VaultChangedPayload,
  VaultEntry,
} from "../types";

export const api = {
  // ---- vault ----
  listVaults: () => invoke<VaultEntry[]>("list_vaults"),
  openVault: (path: string) => invoke<OpenVaultResult>("open_vault", { path }),
  createVault: (path: string) => invoke<OpenVaultResult>("create_vault", { path }),
  closeVault: () => invoke<void>("close_vault"),
  forgetVault: (path: string) => invoke<void>("forget_vault", { path }),

  // ---- 文件 ----
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),
  writeTextFile: (path: string, content: string) =>
    invoke<void>("write_text_file", { path, content }),
  pathExists: (path: string) => invoke<boolean>("path_exists", { path }),
  listDir: (path: string | null) => invoke<FsEntry[]>("list_dir", { path }),
  createNote: (folder: string | null, title: string, content?: string) =>
    invoke<string>("create_note", { folder, title, content: content ?? null }),
  createFolder: (parent: string | null, name: string) =>
    invoke<string>("create_folder", { parent, name }),
  renamePath: (path: string, name: string) => invoke<string>("rename_path", { path, name }),
  deletePath: (path: string) => invoke<void>("delete_path", { path }),
  saveImage: (data: number[], ext: string) => invoke<string>("save_image", { data, ext }),
  writeExternal: (path: string, content: string) =>
    invoke<void>("write_external", { path, content }),

  // ---- 搜索 ----
  searchFiles: (query: string) =>
    invoke<{ path: string; title: string; aliases: string[] }[]>("search_files", { query }),
  searchContent: (query: string) =>
    invoke<{ path: string; title: string; line_no: number; text: string }[]>(
      "search_content",
      { query },
    ),
  searchByTag: (tag: string) =>
    invoke<{ path: string; title: string; aliases: string[] }[]>("search_by_tag", { tag }),

  // ---- 代码索引（检索中心四模式） ----
  codeSymbolSearch: (query: string, kinds: string[] | null, limit: number | null = null) =>
    invoke<SymbolHit[]>("code_symbol_search", { query, kinds, limit }),
  codeFileSymbols: (path: string) => invoke<CodeSymbol[]>("code_file_symbols", { path }),
  codeFileSearch: (query: string, limit: number | null = null) =>
    invoke<CodeFileHit[]>("code_file_search", { query, limit }),
  codeTextSearch: (
    query: string,
    caseSensitive: boolean | null = null,
    maxResults: number | null = null,
  ) => invoke<TextHit[]>("code_text_search", { query, caseSensitive, maxResults }),

  // ---- 文档中心（P2） ----
  docsScan: () => invoke<DocsScanResult>("docs_scan"),
  docsMove: (paths: string[], category: string) =>
    invoke<[string, string][]>("docs_move", { paths, category }),
  docsIgnore: (paths: string[]) => invoke<void>("docs_ignore", { paths }),
  docsUnignore: (path: string | null) => invoke<void>("docs_unignore", { path }),
  docsZip: (entries: [string, string][]) => invoke<string>("docs_zip", { entries }),
  docsGenerateAgents: () => invoke<string>("docs_generate_agents"),
  docsUpdateSettings: (settings: DocsSettingsPatch) =>
    invoke<void>("docs_update_settings", { settings }),
  docsSetMapping: (mapping: Record<string, string>) =>
    invoke<void>("docs_set_mapping", { mapping }),
  docsAiClassify: (paths: string[]) => invoke<number>("docs_ai_classify", { paths }),
  docsQuickOp: (path: string, op: string) => invoke<string>("docs_quick_op", { path, op }),

  // ---- AI Agent（P3） ----
  agentChatStream: (messages: AgentMessage[], system: string) =>
    invoke<number>("agent_chat_stream", { messages, system }),
  agentCancel: (id: number) => invoke<void>("agent_cancel", { id }),
  agentRunCommand: (command: string) => invoke<number>("agent_run_command", { command }),
  agentKillCommand: (id: number) => invoke<void>("agent_kill_command", { id }),
  agentSessionsList: () => invoke<SessionMeta[]>("agent_sessions_list"),
  agentSessionLoad: (id: string) => invoke<SessionDoc>("agent_session_load", { id }),
  agentSessionSave: (session: SessionDoc) => invoke<void>("agent_session_save", { session }),
  agentSessionDelete: (id: string) => invoke<void>("agent_session_delete", { id }),
  toolchainDetect: (kind: string, prefer?: string | null) =>
    invoke<ToolInfo>("toolchain_detect", { kind, prefer: prefer ?? null }),
  onAgentDelta: (cb: (e: { id: number; text: string }) => void): Promise<UnlistenFn> =>
    listen<{ id: number; text: string }>("agent-delta", (ev) => cb(ev.payload)),
  onAgentDone: (cb: (e: { id: number; text: string }) => void): Promise<UnlistenFn> =>
    listen<{ id: number; text: string }>("agent-done", (ev) => cb(ev.payload)),
  onAgentError: (cb: (e: { id: number; error: string } | { id: number; text: string; error?: string }) => void): Promise<UnlistenFn> =>
    listen<{ id: number; error: string }>("agent-error", (ev) => cb(ev.payload)),
  onConsoleOutput: (cb: (e: { id: number; line: string; stderr: boolean }) => void): Promise<UnlistenFn> =>
    listen<{ id: number; line: string; stderr: boolean }>("console-output", (ev) => cb(ev.payload)),
  onConsoleExit: (cb: (e: { id: number; code: number | null }) => void): Promise<UnlistenFn> =>
    listen<{ id: number; code: number | null }>("console-exit", (ev) => cb(ev.payload)),
  onDocsClassifyProgress: (cb: (p: DocsClassifyProgress) => void): Promise<UnlistenFn> =>
    listen<DocsClassifyProgress>("docs-classify-progress", (e) => cb(e.payload)),

  // ---- 索引 ----
  rebuildIndex: () => invoke<NoteIndex[]>("rebuild_index"),
  getAllNotes: () => invoke<NoteIndex[]>("get_all_notes"),
  getTags: () => invoke<TagCount[]>("get_tags"),
  getBacklinks: (path: string) => invoke<Backlink[]>("get_backlinks", { path }),
  resolveLink: (target: string) => invoke<ResolveResult>("resolve_link", { target }),

  // ---- 设置 ----
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<void>("save_settings", { settings }),
  getConfigDirs: () =>
    invoke<{ config_dir: string; snippets_dir: string }>("get_config_dirs"),
  getVaultRoot: () => invoke<string>("get_vault_root"),

  // ---- CSS 片段 ----
  snippetFiles: () => invoke<string[]>("snippet_files"),
  readSnippetFile: (name: string) => invoke<string>("read_snippet_file", { name }),
  createSnippetFile: (name: string) => invoke<void>("create_snippet_file", { name }),
  deleteSnippetFile: (name: string) => invoke<void>("delete_snippet_file", { name }),

  // ---- MCP ----
  mcpTest: (config: McpServerConfig) =>
    invoke<{
      ok: boolean;
      server_name: string;
      tools: { name: string; description: string }[];
      error: string;
    }>("mcp_test", { config }),
  mcpSync: () =>
    invoke<{
      uploaded: string[];
      downloaded: string[];
      skipped: string[];
      errors: string[];
      remote_files: number;
      local_files: number;
    }>("mcp_sync"),

  // ---- AI ----
  aiChat: (prompt: string, system?: string | null) =>
    invoke<string>("ai_chat", { prompt, system: system ?? null }),

  // ---- 导入 ----
  importFiles: (paths: string[], folder: string | null) =>
    invoke<string[]>("import_files", { paths, folder }),
  readExternalBinary: (path: string) => invoke<number[]>("read_external_binary", { path }),
  listAllFilesWithMtime: () =>
    invoke<{ path: string; mtime: number; kind: string }[]>(
      "list_all_files_with_mtime",
    ),
  duplicateFile: (path: string) => invoke<string>("duplicate_file", { path }),
  deleteFiles: (paths: string[]) => invoke<string[]>("delete_files", { paths }),

  // ---- 系统级打开（右键菜单/双击文件） ----
  getPendingFile: () => invoke<string | null>("get_pending_file"),
  openPathFromOs: (path: string) =>
    invoke<{ root: string; notes: NoteIndex[]; rel: string }>("open_path_from_os", { path }),
  openAsTemp: (path: string) =>
    invoke<{ root: string; notes: NoteIndex[]; rel: string }>("open_as_temp", { path }),
  registerContextMenu: () => invoke<void>("register_context_menu"),
  unregisterContextMenu: () => invoke<void>("unregister_context_menu"),
  openDefaultAppsSettings: () => invoke<void>("open_default_apps_settings"),
  onOsOpenFile: (handler: (path: string) => void): Promise<UnlistenFn> =>
    listen<string>("os-open-file", (e) => handler(e.payload)),

  // ---- 事件 ----
  onVaultChanged: (handler: (payload: VaultChangedPayload) => void): Promise<UnlistenFn> =>
    listen<VaultChangedPayload>("vault-changed", (e) => handler(e.payload)),
};

/** vault 内资源 → emdasset URL（Rust 端校验只读 vault 内文件） */
export function assetUrl(relPath: string): string {
  return `emdasset://vault/${encodeURIComponent(relPath)}`;
}
