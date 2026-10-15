// md 文本工具（与 Rust parser 行为对齐的 JS 版）—— 纯字符串处理，零依赖
// 性能：markdown-it 已拆到 ./md-render，本文件保持无重依赖，
// 让首屏主包（EditorPane / PropertiesPanel 等）不被迫带上 markdown-it。

export interface RenderOpts {
  /** [[目标]] → 笔记相对路径；未解析返回 null */
  resolveNote: (target: string) => string | null;
  /** 附件目录（无路径的图片名兜底） */
  attachmentsDir: string;
  /** 当前笔记相对路径（解析相对资源用） */
  sourcePath?: string;
}

export function emdAssetUrl(relPath: string): string {
  return `emdasset://vault/${encodeURIComponent(relPath)}`;
}

// ---------- 文本工具（与 Rust parser 行为对齐的 JS 版） ----------

export function stripFrontmatter(text: string): { fm: string | null; body: string; bodyStartLine: number } {
  const t = text.startsWith("\u{feff}") ? text.slice(1) : text;
  if (t.startsWith("---\n") || t.startsWith("---\r\n")) {
    const nl = t.startsWith("---\r\n") ? 5 : 4;
    const rest = t.slice(nl);
    const m = /^(---|\.\.\.)\s*(\r?\n|$)/m.exec(rest);
    if (m && m.index !== undefined) {
      const fm = rest.slice(0, m.index);
      const body = rest.slice(m.index + m[0].length);
      return { fm, body, bodyStartLine: fm.split("\n").length + 1 };
    }
  }
  return { fm: null, body: t, bodyStartLine: 0 };
}

/** 从 body 提取某标题的章节（到同级或更高级标题为止） */
export function extractSection(body: string, heading: string): string {
  const lines = body.split("\n");
  let start = -1;
  let level = 0;
  const headingNorm = heading.trim().toLowerCase();
  for (let i = 0; i < lines.length; i++) {
    const m = /^(#{1,6})\s+(.*)$/.exec(lines[i]);
    if (m && m[2].trim().toLowerCase() === headingNorm) {
      start = i + 1;
      level = m[1].length;
      break;
    }
  }
  if (start === -1) return "";
  const out: string[] = [];
  for (let i = start; i < lines.length; i++) {
    const m = /^(#{1,6})\s+/.exec(lines[i]);
    if (m && m[1].length <= level) break;
    out.push(lines[i]);
  }
  return out.join("\n").trim();
}

/** 提取块 id 对应的块内容（^id 所在的空行分隔块） */
export function extractBlock(body: string, blockId: string): string {
  const lines = body.split("\n");
  let hit = -1;
  for (let i = 0; i < lines.length; i++) {
    const m = /\^([A-Za-z0-9-]+)\s*$/.exec(lines[i]);
    if (m && m[1] === blockId) {
      hit = i;
      break;
    }
  }
  if (hit === -1) return "";
  let start = hit;
  while (start > 0 && lines[start - 1].trim() !== "") start--;
  let end = hit;
  while (end < lines.length - 1 && lines[end + 1].trim() !== "") end++;
  return lines
    .slice(start, end + 1)
    .join("\n")
    .replace(/\s*\^[A-Za-z0-9-]+\s*$/, "")
    .trim();
}

export function slugify(text: string): string {
  return (
    text
      .trim()
      .toLowerCase()
      .replace(/[\s]+/g, "-")
      .replace(/[^\p{L}\p{N}\-_]/gu, "") || "h"
  );
}

/** 行内标题解析（大纲面板用） */
export function parseHeadings(text: string): { level: number; text: string; line: number }[] {
  const { body, bodyStartLine } = stripFrontmatter(text);
  const out: { level: number; text: string; line: number }[] = [];
  body.split("\n").forEach((line, i) => {
    const m = /^ {0,3}(#{1,6})[ \t]+(\S.*)$/.exec(line);
    if (m) {
      out.push({
        level: m[1].length,
        text: m[2].trim().replace(/\s*#+\s*$/, ""),
        line: i + bodyStartLine,
      });
    }
  });
  return out;
}
