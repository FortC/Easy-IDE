// 文件类型注册表：扩展名 → CM6 语言包（按需动态加载）、图标、可编辑性。
// P0 范围：高亮走 CM6 官方语言包 + legacy-modes；符号索引在 P1 由 Rust 侧提供。
import type { Extension } from "@codemirror/state";
import { StreamLanguage } from "@codemirror/language";

/** legacy-modes 的 StreamParser 与 language 包存在类型版本偏差（运行时兼容），这里统一断言 */
type LegacyMode = Parameters<typeof StreamLanguage.define>[0];
const asMode = (m: unknown): LegacyMode => m as LegacyMode;

export interface CodeLangSpec {
  /** 语言标识（用于显示与后续符号索引） */
  id: string;
  /** 动态加载 CM6 语言扩展（Vite 自动分包，首次打开该类型文件才下载） */
  load: () => Promise<Extension[]>;
  /** 文件树/标签页图标（iconPaths 中的名称） */
  icon: string;
}

/** 代码语言映射（不含 markdown，markdown 走原有编辑链路） */
const LANGS: Record<string, CodeLangSpec> = {
  java: {
    id: "java",
    icon: "coffee",
    load: async () => [(await import("@codemirror/lang-java")).java()],
  },
  xml: {
    id: "xml",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-xml")).xml()],
  },
  json: {
    id: "json",
    icon: "braces",
    load: async () => [(await import("@codemirror/lang-json")).json()],
  },
  javascript: {
    id: "javascript",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-javascript")).javascript()],
  },
  typescript: {
    id: "typescript",
    icon: "file-code",
    load: async () => [
      (await import("@codemirror/lang-javascript")).javascript({ typescript: true }),
    ],
  },
  tsx: {
    id: "tsx",
    icon: "file-code",
    load: async () => [
      (await import("@codemirror/lang-javascript")).javascript({ typescript: true, jsx: true }),
    ],
  },
  jsx: {
    id: "jsx",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-javascript")).javascript({ jsx: true })],
  },
  python: {
    id: "python",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-python")).python()],
  },
  cpp: {
    id: "cpp",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-cpp")).cpp()],
  },
  html: {
    id: "html",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-html")).html()],
  },
  css: {
    id: "css",
    icon: "file-code",
    load: async () => [(await import("@codemirror/lang-css")).css()],
  },
  sql: {
    id: "sql",
    icon: "database",
    load: async () => [(await import("@codemirror/lang-sql")).sql()],
  },
  yaml: {
    id: "yaml",
    icon: "settings-2",
    load: async () => [
      StreamLanguage.define(
        asMode((await import("@codemirror/legacy-modes/mode/yaml")).yaml),
      ),
    ],
  },
  toml: {
    id: "toml",
    icon: "settings-2",
    load: async () => [
      StreamLanguage.define(
        asMode((await import("@codemirror/legacy-modes/mode/toml")).toml),
      ),
    ],
  },
  properties: {
    id: "properties",
    icon: "settings-2",
    load: async () => [
      StreamLanguage.define(
        asMode((await import("@codemirror/legacy-modes/mode/properties")).properties),
      ),
    ],
  },
  shell: {
    id: "shell",
    icon: "terminal",
    load: async () => [
      StreamLanguage.define(
        asMode((await import("@codemirror/legacy-modes/mode/shell")).shell),
      ),
    ],
  },
};

/** 扩展名 → 语言规格键 */
const EXT_MAP: Record<string, keyof typeof LANGS> = {
  java: "java",
  xml: "xml",
  json: "json",
  mjs: "javascript",
  cjs: "javascript",
  ts: "typescript",
  tsx: "tsx",
  jsx: "jsx",
  mts: "typescript",
  cts: "typescript",
  py: "python",
  pyw: "python",
  c: "cpp",
  h: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  cc: "cpp",
  hpp: "cpp",
  hh: "cpp",
  ino: "cpp",
  html: "html",
  htm: "html",
  vue: "html",
  svelte: "html",
  astro: "html",
  css: "css",
  scss: "css",
  less: "css",
  sql: "sql",
  yml: "yaml",
  yaml: "yaml",
  toml: "toml",
  properties: "properties",
  ini: "properties",
  conf: "properties",
  cfg: "properties",
  env: "properties",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
};

/** 直接可编辑的纯文本扩展名（无高亮，但有行号/等宽字体） */
const PLAIN_EXTS = new Set([
  "txt", "log", "log4j", "md5", "csv", "diff", "patch", "bat", "cmd", "gradle", "kts",
  "gitignore", "gitattributes", "editorconfig", "mvn", "pro", "lock",
]);

export function extOf(path: string): string {
  const name = path.replace(/\\/g, "/").split("/").pop() || "";
  const idx = name.lastIndexOf(".");
  if (idx <= 0) return ""; // 无扩展名或点开头的文件（.gitignore 等）
  return name.slice(idx + 1).toLowerCase();
}

/** markdown（含无扩展名按 md 处理的历史行为不存在：这里只认扩展名） */
export function isMarkdown(path: string): boolean {
  const e = extOf(path);
  return e === "md" || e === "markdown";
}

/** 代码文件（非 markdown，有 CM6 语言规格） */
export function codeLangOf(path: string): CodeLangSpec | null {
  const key = EXT_MAP[extOf(path)];
  return key ? LANGS[key] : null;
}

/** 是否为可在编辑器打开的文本文件（md / 代码 / 纯文本） */
export function isTextEditable(path: string): boolean {
  if (isMarkdown(path)) return true;
  if (codeLangOf(path)) return true;
  return PLAIN_EXTS.has(extOf(path));
}

/** 文件树 / 标签页图标 */
export function iconForFile(path: string): string {
  const name = path.replace(/\\/g, "/").split("/").pop() || "";
  if (name === "pom.xml" || name === "build.gradle" || name === "build.gradle.kts") return "package";
  if (isMarkdown(path)) return "file-text";
  if (name.toLowerCase().endsWith(".canvas")) return "layout-grid";
  if (name.toLowerCase().endsWith(".graph")) return "share-2";
  const e = extOf(path);
  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"].includes(e)) return "image";
  return codeLangOf(path)?.icon ?? "file-text";
}

/** 编译产物/依赖目录：文件树中置灰显示 */
export const BUILD_DIRS = new Set([
  "target", "build", "dist", "out", "node_modules", "vendor", ".gradle", ".cache",
  "cmake-build-debug", "cmake-build-release", "__pycache__",
]);
