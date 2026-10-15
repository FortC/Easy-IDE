// markdown-it 渲染管线：wikilink / 标签 / 图片资源改写 / 标题锚点
// 性能：markdown-it 仅在真正需要渲染 md 时才载入（懒加载），
// 首屏主包不含 markdown-it —— 文本工具函数在 renderer.ts（纯字符串，无依赖）。
import MarkdownIt from "markdown-it";
import type { RenderOpts } from "./renderer";
import { slugify, stripFrontmatter } from "./renderer";
import { t } from "../../i18n";

const md: MarkdownIt = new MarkdownIt({ html: false, linkify: true, breaks: false });

// ---------- wikilink / embed ----------
md.inline.ruler.before("link", "emd_wikilink", (state, silent) => {
  const src = state.src;
  let pos = state.pos;
  let embed = false;
  if (src[pos] === "!") {
    embed = true;
    pos++;
  }
  if (src[pos] !== "[" || src[pos + 1] !== "[") return false;
  const end = src.indexOf("]]", pos + 2);
  if (end === -1 || end > state.posMax) return false;
  const inner = src.slice(pos + 2, end);
  if (inner.includes("[") || inner.includes("\n")) return false;

  if (!silent) {
    let main = inner;
    let alias: string | null = null;
    const bar = inner.indexOf("|");
    if (bar !== -1) {
      main = inner.slice(0, bar);
      alias = inner.slice(bar + 1).trim();
    }
    main = main.trim();
    let target = main;
    let subpath: string | null = null;
    const hash = main.indexOf("#");
    if (hash !== -1) {
      target = main.slice(0, hash).trim();
      subpath = main.slice(hash + 1);
    }
    const token = state.push(embed ? "emd_embed" : "emd_wikilink", "", 0);
    token.meta = { target, subpath, alias };
    token.content = state.src.slice(state.pos, end + 2);
  }
  state.pos = end + 2;
  return true;
});

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
function escAttr(s: string): string {
  return escHtml(s).replace(/'/g, "&#39;");
}

// ---------- 标签 #tag ----------
md.inline.ruler.before("emphasis", "emd_tag", (state, silent) => {
  const src = state.src;
  const pos = state.pos;
  if (src[pos] !== "#") return false;
  // 前面必须是行首或空白
  if (pos > 0 && !/[\s(\u3000]/.test(src[pos - 1])) return false;
  const m = /^#([\p{L}\p{N}_][\p{L}\p{N}_/-]*)/u.exec(src.slice(pos));
  if (!m) return false;
  const tag = m[1].replace(/\/+$/, "");
  if (!tag || /^\d+$/.test(tag)) return false;
  if (!silent) {
    const token = state.push("emd_tag", "", 0);
    token.content = tag;
  }
  state.pos = pos + m[0].length;
  return true;
});

// ---------- 高亮 ==text== ----------
md.inline.ruler.before("emphasis", "emd_mark", (state, silent) => {
  const src = state.src;
  const pos = state.pos;
  if (src[pos] !== "=" || src[pos + 1] !== "=") return false;
  const end = src.indexOf("==", pos + 2);
  if (end === -1 || end === pos + 2 || end > state.posMax) return false;
  const inner = src.slice(pos + 2, end);
  if (inner.includes("\n") || inner.includes("=")) return false;
  if (!silent) {
    const token = state.push("emd_mark", "", 0);
    token.content = inner;
  }
  state.pos = end + 2;
  return true;
});

md.renderer.rules.emd_mark = (tokens, idx) => {
  return `<mark class="md-mark">${escHtml(tokens[idx].content)}</mark>`;
};

// ---------- 图片资源改写（相对路径 → emdasset） ----------
md.renderer.rules.image = (tokens, idx) => {
  const tok = tokens[idx];
  const rawSrc = tok.attrGet("src") || "";
  const alt = escAttr(tok.content || "");
  let src = rawSrc;
  if (!/^(https?:|emdasset:|data:|blob:)/i.test(rawSrc) && rawSrc) {
    src = emdAssetUrl(rawSrc.startsWith("/") ? rawSrc.slice(1) : rawSrc);
  }
  return `<img src="${escAttr(src)}" alt="${alt}" loading="lazy" />`;
};

// ---------- 标题锚点 ----------
md.renderer.rules.heading_open = (tokens, idx) => {
  const tok = tokens[idx];
  const inline = tokens[idx + 1];
  const slug = slugify(inline ? inline.content : "");
  return `<${tok.tag} id="emd-h-${slug}" data-heading="${escAttr(inline ? inline.content : "")}">`;
};

// ---------- 默认规则绑定（渲染时机注入 resolve 上下文） ----------
let CURRENT: RenderOpts | null = null;

md.renderer.rules.emd_wikilink = (tokens, idx) => {
  const { target, subpath, alias } = tokens[idx].meta;
  const label =
    alias ||
    (target ? target + (subpath ? `#${subpath}` : "") : subpath || t("pv.self2"));
  const resolved = CURRENT?.resolveNote(target || "") ?? null;
  const cls = resolved ? "internal-link" : "internal-link is-unresolved";
  return `<a class="${cls}" data-target="${escAttr(target)}" data-subpath="${escAttr(subpath || "")}" href="javascript:void(0)">${escHtml(label)}</a>`;
};

md.renderer.rules.emd_embed = (tokens, idx) => {
  const { target, subpath } = tokens[idx].meta;
  return `<div class="emd-embed" data-embed-target="${escAttr(target)}" data-embed-sub="${escAttr(subpath || "")}"></div>`;
};

md.renderer.rules.emd_tag = (tokens, idx) => {
  return `<a class="md-tag" data-tag="${escAttr(tokens[idx].content)}" href="javascript:void(0)">#${escHtml(tokens[idx].content)}</a>`;
};

export function emdAssetUrl(relPath: string): string {
  return `emdasset://vault/${encodeURIComponent(relPath)}`;
}

/** 渲染入口（同步渲染，嵌入内容由 PreviewView 异步填充） */

/** 渲染入口（同步渲染，嵌入内容由 PreviewView 异步填充） */
export function renderMarkdown(content: string, opts: RenderOpts): string {
  CURRENT = opts;
  try {
    const body = stripFrontmatter(content).body;
    let html = md.render(body);
    // 任务列表复选框
    html = html.replace(
      /<li>(\s*)\[([ xX])\]\s/g,
      (_all, sp, mark) =>
        `<li><input type="checkbox" class="md-task" disabled ${mark.trim() ? "checked" : ""}>${sp ? "" : " "}`,
    );
    return html;
  } finally {
    CURRENT = null;
  }
}

/**
 * 异步渲染（性能友好版）：首次调用时才 import markdown-it
 * 首屏主包因此不含 markdown-it；返回的 Promise 在载入完成后 resolve。
 * 用于首屏路径（AI 消息渲染 / md 预览）的按需渲染。
 */
let modPromise: Promise<typeof import("./md-render")> | null = null;
export function renderMarkdownAsync(
  content: string,
  opts: RenderOpts,
): Promise<string> {
  if (!modPromise) modPromise = import("./md-render");
  return modPromise.then((m) => m.renderMarkdown(content, opts));
}

/** 预热：提前载入 markdown-it（空闲时调用，避免首次渲染等待） */
export function preloadMarkdown(): void {
  if (!modPromise) modPromise = import("./md-render");
}

