/**
 * B7 · A 档：本地符号联想引擎（零依赖、零模型、不联网）。
 *
 * 为什么不是「内置一个轻量 AI」：应用内塞 GGUF 权重意味着数百 MB 包体 + 推理引擎，
 * 而这个仓库的定位是桌面 IDE 的骨架。真要语义能力，走 B 档接本地 Ollama 端点即可。
 * A 档解决的是最高频、最刚需的那一半诉求 —— **打字时的符号联想与快速解析**，
 * 而这部分靠工作区已有的 `code_index` 就够了（它已做前缀/词边界/包含三级排序 + camelHump 分词）。
 *
 * 设计要点：
 *  - 结果**本地缓存**。索引刷新才有新符号，所以按 `(path, 符号集版本)` 缓存，
 *    否则每敲一个字符都要跨 IPC 打一次 Rust。
 *  - **同文件优先**。光标所在文件的符号是最强上下文，直接顶到列表最前。
 *  - 排序不是照搬 `codeSymbolSearch`：联想场景下「作用域内」比「前缀匹配」更重要。
 */

import { api } from "../ipc/tauri";
import type { CodeSymbol } from "../types";

/** 联想结果上限。太多会淹没列表，20 条足够覆盖绝大多数输入前缀 */
const MAX_SUGGEST = 20;
/** 前缀至少这么长才联想，避免单字符触发噪声（打字第一下就会弹，纯打扰） */
const MIN_PREFIX = 1;
/** 缓存有效期：符号索引不会在用户敲键盘的过程中变化，2s 足够覆盖索引延迟 */
const CACHE_TTL = 2000;

export interface LocalSuggestion {
  label: string;
  kind: string;
  detail: string;
  /** 桶序号，供排序用；仅本模块使用，不进 CodeMirror Completion */
  bucket: number;
}

/** 按文件路径缓存该文件的符号列表 */
const fileSymbolCache = new Map<string, { at: number; symbols: CodeSymbol[] }>();
/** 全工作区符号查询缓存：key = 小写查询词（不同前缀共享后缀命中不多，简单粗暴即可） */
const queryCache = new Map<string, { at: number; hits: Awaited<ReturnType<typeof api.codeSymbolSearch>> }>();

function fresh<T>(e: { at: number } | undefined): e is { at: number } & T {
  return !!e && Date.now() - e.at < CACHE_TTL;
}

/** 符号类型 → 中文短语，用于补全项右侧 detail */
const KIND_LABEL: Record<string, string> = {
  class: "类",
  interface: "接口",
  enum: "枚举",
  record: "记录",
  function: "函数",
  method: "方法",
  field: "字段",
  property: "属性",
  variable: "变量",
  constant: "常量",
  struct: "结构体",
  trait: "trait",
  fn: "函数",
};

/**
 * 联想入口。
 * @param prefix 用户已输入的前缀（光标前的标识符片段）
 * @param currentPath 当前文件路径，用于「同文件优先」
 */
export async function suggestSymbols(
  prefix: string,
  currentPath: string,
): Promise<LocalSuggestion[]> {
  const q = prefix.trim();
  if (q.length < MIN_PREFIX) return [];

  // 同文件符号：命中即最高优先，且成本只是一次 IPC（且有缓存）
  const local = await fileSymbols(currentPath);
  const out: LocalSuggestion[] = [];
  const seen = new Set<string>();

  const push = (label: string, kind: string, detail: string, bucket: number) => {
    if (!label || seen.has(label)) return;
    seen.add(label);
    out.push({ label, kind, detail, bucket });
  };

  for (const s of local) {
    if (!s.name.toLowerCase().startsWith(q.toLowerCase())) continue;
    // 完全等长的多半是用户自己已经在写的名字，价值最低，排最后
    const exact = s.name.length === q.length;
    push(s.name, s.kind || "symbol", describe(s, KIND_LABEL), exact ? 4 : (s.kind === "class" || s.kind === "interface" ? 0 : 2));
  }

  // 全工作区补足
  if (out.length < MAX_SUGGEST) {
    for (const h of await searchAll(q)) {
      if (out.length >= MAX_SUGGEST) break;
      const bucket = h.name.toLowerCase() === q.toLowerCase()
        ? 5
        : h.name.toLowerCase().startsWith(q.toLowerCase())
          ? 3
          : 4;
      // 同文件已给过的靠 seen 去重；detail 带上文件名，便于区分重名符号
      push(h.name, h.kind || "symbol", `${h.container || h.path} · ${KIND_LABEL[h.kind] || h.kind || "符号"}`, bucket + 1);
    }
  }

  out.sort((a, b) => a.bucket - b.bucket || a.label.length - b.label.length || a.label.localeCompare(b.label));
  return out.slice(0, MAX_SUGGEST);
}

function describe(s: CodeSymbol, labels: Record<string, string>): string {
  const kind = labels[s.kind] || s.kind || "符号";
  return s.container ? `${s.container} · ${kind}` : kind;
}

async function fileSymbols(path: string): Promise<CodeSymbol[]> {
  const hit = fileSymbolCache.get(path);
  if (fresh(hit)) return hit.symbols;
  try {
    const symbols = await api.codeFileSymbols(path);
    fileSymbolCache.set(path, { at: Date.now(), symbols });
    return symbols;
  } catch {
    // 文件被删/未索引：不能抛给补全流程，否则会打断打字
    return [];
  }
}

async function searchAll(q: string) {
  const key = q.toLowerCase();
  const hit = queryCache.get(key);
  if (fresh(hit)) return hit.hits;
  try {
    const hits = await api.codeSymbolSearch(q, null, MAX_SUGGEST * 3);
    queryCache.set(key, { at: Date.now(), hits });
    return hits;
  } catch {
    return [];
  }
}

/** 索引刷新 / 切换工作区后调用，避免展示已删除的符号 */
export function invalidateLocalSuggest(): void {
  fileSymbolCache.clear();
  queryCache.clear();
}
