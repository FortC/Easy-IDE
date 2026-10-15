/**
 * Alt+点击跳定义（B5.1）
 *
 * 后端能力已完备：code_symbol_search 返回 { path, line }，code_file_symbols 返回单文件符号表。
 * 本模块只负责「光标处取词 → 解析目标位置 → 消歧排序」，不含任何 UI。
 *
 * 为什么不引入 @codemirror/language 的 wordAt：
 * 6.12.4 未导出该函数，且语法树方案对未加载语言包的文件会失效（纯文本退化成无节点）。
 * 标识符正则对 java/js/ts/py 等主流语言都成立，且零新增依赖（守首屏主包预算）。
 */
import { api } from "../ipc/tauri";
import type { SymbolHit } from "../types";

/** 标识符：字母/数字/下划线/$，首字符不含数字（跨 java/js/ts/py 通用） */
const RE_IDENT = /[\p{L}_$][\p{L}\p{N}_$]*/u;

/** 解析优先级用：优先跳到类型定义，再是成员 */
const KIND_RANK: Record<string, number> = {
  class: 0,
  interface: 0,
  enum: 0,
  record: 0,
  function: 1,
  method: 1,
  field: 2,
};

/** 同名去重时保留的候选上限 */
const MAX_CANDIDATES = 30;
/** 超过这个数就不弹选择器了（同名方法到处都是，无意义的噪音） */
const MAX_PICK = 12;

/**
 * 从文档文本中取出 pos 处的标识符。
 * 纯文本扫描，不依赖语法树 —— 语言包未就绪时同样可用。
 *
 * ⚠️ 必须从 pos 向两侧扩展找完整词，不能只从 pos 往后切片匹配：
 * 点击 `Identity` 的中间位置时，往后切片必然从残词（entity/tity）开始匹配，
 * 会拿到半个词，符号检索随之落空。
 */
export function wordAtPos(text: string, pos: number): string | null {
  if (pos < 0 || pos > text.length) return null;
  const isIdent = (ch: string) => /[\p{L}\p{N}_$]/u.test(ch);

  // 光标紧贴词尾时（右侧点击/方向键落点）先左移一格，避免漏掉紧邻的词
  let p = Math.min(pos, text.length - 1);
  if (p < 0) return null;
  if (!isIdent(text[p]) && isIdent(text[p - 1] ?? "")) p -= 1;
  if (!isIdent(text[p])) return null;

  let start = p;
  while (start > 0 && isIdent(text[start - 1])) start -= 1;
  let end = p + 1;
  while (end < text.length && isIdent(text[end])) end += 1;
  return text.slice(start, end) || null;
}

export interface GotoTarget {
  path: string;
  /** 1 基行号，与后端 SymbolHit 一致 */
  line: number;
  kind: string;
  /** 所属外层类型/函数，用于选择器里区分同名成员 */
  container: string | null;
  name: string;
}

function fromHit(h: SymbolHit): GotoTarget {
  return { path: h.path, line: h.line, kind: h.kind, container: h.container, name: h.name };
}

/** 同 path + 同 line 视为同一处定义 */
function dedupeKey(t: GotoTarget): string {
  return `${t.path}::${t.line}::${t.name}`;
}

/**
 * 解析跳转目标。
 *
 * 策略（与方案文档 B5.1 一致）：
 *  1. 先查当前文件 —— 最快最准，且不会因为索引里存在同名类而跳错文件
 *  2. 未命中再全工作区符号检索
 *  3. 多命中按 类型 > 函数 > 字段 排序后交给 UI 消歧；单命中直接跳
 *
 * 返回空数组表示「没找到」，UI 需给出提示而不是静默失败。
 */
export async function resolveDefinition(
  word: string,
  currentPath: string,
): Promise<GotoTarget[]> {
  if (!word) return [];

  // 1. 当前文件优先
  if (currentPath) {
    try {
      const syms = await api.codeFileSymbols(currentPath);
      const local = syms
        .filter((s) => s.name === word)
        .map((s): GotoTarget => ({
          path: currentPath,
          line: s.line,
          kind: s.kind,
          container: s.container,
          name: s.name,
        }));
      if (local.length) {
        // 当前文件里唯一命中 → 直接跳，不打扰用户
        if (local.length === 1) return local;
        return rank(local).slice(0, MAX_PICK);
      }
    } catch {
      // 索引尚未建立 / 文件被删，静默降级到全局检索
    }
  }

  // 2. 全工作区符号检索
  let hits: SymbolHit[];
  try {
    hits = await api.codeSymbolSearch(word, null, MAX_CANDIDATES);
  } catch {
    return [];
  }
  if (!hits.length) return [];

  // 3. 精确同名优先 —— 检索是包含匹配，"User" 会带出 "UserService"
  const exact = hits.filter((h) => h.name === word);
  const pool = exact.length ? exact : hits;
  const targets = pool.map(fromHit);
  const seen = new Set<string>();
  const uniq = targets.filter((t) => {
    const k = dedupeKey(t);
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  return rank(uniq).slice(0, MAX_PICK);
}

function rank(list: GotoTarget[]): GotoTarget[] {
  return list.sort((a, b) => {
    const r = (KIND_RANK[a.kind] ?? 3) - (KIND_RANK[b.kind] ?? 3);
    if (r !== 0) return r;
    const c = a.container?.localeCompare(b.container ?? "") ?? 0;
    if (c !== 0) return c;
    return a.path.localeCompare(b.path);
  });
}

/** 超过此数量说明同名命中过多（如 Object.toString），不值得让用户选 */
export function shouldPick(list: GotoTarget[]): boolean {
  return list.length > 1;
}