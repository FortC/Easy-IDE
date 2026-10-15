// 代码配色主题注册表 —— 语法高亮色值的唯一来源（UI 色值仍走 theme.css）。
// 每套主题产出一组 --code-* CSS 变量挂到编辑器宿主元素上；
// 高亮样式与编辑器 chrome 统一引用这些变量，切换主题 = 重设变量，零重建。
import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import type { Extension } from "@codemirror/state";
import { t } from "../i18n";

/** 一套主题的全部色值 */
export interface CodeThemeColors {
  bg: string;
  fg: string;
  gutterFg: string;
  border: string;
  line: string;
  selection: string;
  cursor: string;
  bracket: string;
  comment: string;
  keyword: string;
  keyword2: string;
  str: string;
  escape: string;
  regexp: string;
  number: string;
  type: string;
  func: string;
  variable: string;
  constant: string;
  operator: string;
  punctuation: string;
  meta: string;
  tag: string;
  attr: string;
  invalid: string;
  link: string;
  heading: string;
}

export interface CodeTheme {
  key: string;
  label: () => string;
  /** 深/浅，影响编辑器 dark 标记与选择逻辑 */
  mode: "dark" | "light";
  colors: CodeThemeColors;
  /** 设置页色块预览：[背景, 关键字, 字符串, 类型, 注释] */
  swatch: [string, string, string, string, string];
}

/** 内置 5 套：两深、两浅、一高对比 */
export const CODE_THEMES: CodeTheme[] = [
  {
    key: "deep",
    label: () => t("ct.deep"),
    mode: "dark",
    swatch: ["#1e1e1e", "#c586c0", "#ce9178", "#4ec9b0", "#6a9955"],
    colors: {
      bg: "#1e1e1e",
      fg: "#d4d4d4",
      gutterFg: "#858585",
      border: "#333333",
      line: "#262626",
      selection: "#264f78",
      cursor: "#c5c5c5",
      bracket: "#3a3d41",
      comment: "#6a9955",
      keyword: "#c586c0",
      keyword2: "#569cd6",
      str: "#ce9178",
      escape: "#d7ba7d",
      regexp: "#d16969",
      number: "#b5cea8",
      type: "#4ec9b0",
      func: "#dcdcaa",
      variable: "#9cdcfe",
      constant: "#4fc1ff",
      operator: "#d4d4d4",
      punctuation: "#d4d4d4",
      meta: "#dcdcaa",
      tag: "#569cd6",
      attr: "#9cdcfe",
      invalid: "#f44747",
      link: "#569cd6",
      heading: "#569cd6",
    },
  },
  {
    key: "midnight",
    label: () => t("ct.midnight"),
    mode: "dark",
    swatch: ["#1a1b26", "#bb9af7", "#9ece6a", "#2ac3de", "#565f89"],
    colors: {
      bg: "#1a1b26",
      fg: "#c0caf5",
      gutterFg: "#565f89",
      border: "#2a2e3f",
      line: "#20222e",
      selection: "#33467c",
      cursor: "#c0caf5",
      bracket: "#3b4261",
      comment: "#565f89",
      keyword: "#bb9af7",
      keyword2: "#7aa2f7",
      str: "#9ece6a",
      escape: "#89ddff",
      regexp: "#ff9e64",
      number: "#ff9e64",
      type: "#2ac3de",
      func: "#7dcfff",
      variable: "#c0caf5",
      constant: "#ff9e64",
      operator: "#89ddff",
      punctuation: "#a9b1d6",
      meta: "#7aa2f7",
      tag: "#f7768e",
      attr: "#73daca",
      invalid: "#f7768e",
      link: "#73daca",
      heading: "#7aa2f7",
    },
  },
  {
    key: "paper",
    label: () => t("ct.paper"),
    mode: "light",
    swatch: ["#ffffff", "#cf222e", "#0a3069", "#8250df", "#6e7781"],
    colors: {
      bg: "#ffffff",
      fg: "#24292f",
      gutterFg: "#6e7781",
      border: "#d0d7de",
      line: "#f6f8fa",
      selection: "#b6e3ff",
      cursor: "#24292f",
      bracket: "#fff8c5",
      comment: "#6e7781",
      keyword: "#cf222e",
      keyword2: "#cf222e",
      str: "#0a3069",
      escape: "#0a3069",
      regexp: "#a40e26",
      number: "#0550ae",
      type: "#8250df",
      func: "#8250df",
      variable: "#953800",
      constant: "#0550ae",
      operator: "#cf222e",
      punctuation: "#57606a",
      meta: "#8250df",
      tag: "#116329",
      attr: "#0550ae",
      invalid: "#82071e",
      link: "#0969da",
      heading: "#0550ae",
    },
  },
  {
    key: "warm",
    label: () => t("ct.warm"),
    mode: "light",
    swatch: ["#fdf6e3", "#cb4b16", "#2aa198", "#268bd2", "#93a1a1"],
    colors: {
      bg: "#fdf6e3",
      fg: "#586e75",
      gutterFg: "#93a1a1",
      border: "#eee8d5",
      line: "#eee8d5",
      selection: "#eee8d5",
      cursor: "#586e75",
      bracket: "#eee8d5",
      comment: "#93a1a1",
      keyword: "#cb4b16",
      keyword2: "#cb4b16",
      str: "#2aa198",
      escape: "#2aa198",
      regexp: "#dc322f",
      number: "#6c71c4",
      type: "#268bd2",
      func: "#b58900",
      variable: "#586e75",
      constant: "#6c71c4",
      operator: "#859900",
      punctuation: "#586e75",
      meta: "#b58900",
      tag: "#268bd2",
      attr: "#b58900",
      invalid: "#dc322f",
      link: "#268bd2",
      heading: "#268bd2",
    },
  },
  {
    key: "contrast",
    label: () => t("ct.contrast"),
    mode: "dark",
    swatch: ["#000000", "#ffe000", "#00e676", "#b388ff", "#76c776"],
    colors: {
      bg: "#000000",
      fg: "#ffffff",
      gutterFg: "#cccccc",
      border: "#666666",
      line: "#1f1f1f",
      selection: "#33445c",
      cursor: "#ffffff",
      bracket: "#ffe000",
      comment: "#76c776",
      keyword: "#ffe000",
      keyword2: "#ffb300",
      str: "#00e676",
      escape: "#ff8a80",
      regexp: "#ff8a80",
      number: "#40c4ff",
      type: "#b388ff",
      func: "#ff8a65",
      variable: "#ffffff",
      constant: "#40c4ff",
      operator: "#f5f5f5",
      punctuation: "#cfcfcf",
      meta: "#ff80ab",
      tag: "#4dd0e1",
      attr: "#ffd54f",
      invalid: "#ff1744",
      link: "#40c4ff",
      heading: "#ffe000",
    },
  },
];

/** 「跟随界面」：深色界面用深邃黑，浅色界面用亮白清爽 */
export function resolveCodeTheme(pref: string, uiTheme: string): CodeTheme {
  if (!pref || pref === "auto") {
    return CODE_THEMES.find((th) => th.key === (uiTheme === "light" ? "paper" : "deep")) ?? CODE_THEMES[0];
  }
  return CODE_THEMES.find((th) => th.key === pref) ?? CODE_THEMES[0];
}

/** 把主题色值写成 --code-* 变量挂到宿主元素（md 模式传 null 清除，回落 UI 令牌） */
export function applyCodeThemeVars(el: HTMLElement | null, theme: CodeTheme | null) {
  if (!el) return;
  const map: Record<string, string> = {
    bg: "--code-bg",
    fg: "--code-fg",
    gutterFg: "--code-gutter-fg",
    border: "--code-border",
    line: "--code-line",
    selection: "--code-selection",
    cursor: "--code-cursor",
    bracket: "--code-bracket",
    comment: "--code-comment",
    keyword: "--code-kw",
    keyword2: "--code-kw2",
    str: "--code-str",
    escape: "--code-esc",
    regexp: "--code-re",
    number: "--code-num",
    type: "--code-type",
    func: "--code-func",
    variable: "--code-var",
    constant: "--code-const",
    operator: "--code-op",
    punctuation: "--code-punct",
    meta: "--code-meta",
    tag: "--code-tag",
    attr: "--code-attr",
    invalid: "--code-invalid",
    link: "--code-link",
    heading: "--code-heading",
  };
  for (const [key, cssVar] of Object.entries(map)) {
    const v = theme ? theme.colors[key as keyof CodeThemeColors] : "";
    if (v) el.style.setProperty(cssVar, v);
    else el.style.removeProperty(cssVar);
  }
  if (theme) el.dataset.codeThemeMode = theme.mode;
  else delete el.dataset.codeThemeMode;
}

/**
 * 代码文件的编辑器扩展：chrome + 语法高亮，全部引用 --code-* 变量。
 * 只给代码文件用（md 保持默认高亮 + UI 令牌）。
 */
export function codeThemeExtensions(): Extension[] {
  const chrome = EditorView.theme({
    "&": { backgroundColor: "var(--code-bg)", color: "var(--code-fg)" },
    ".cm-content": { caretColor: "var(--code-cursor)" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--code-cursor)" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, .cm-content ::selection": {
      backgroundColor: "var(--code-selection)",
    },
    ".cm-selectionMatch": { backgroundColor: "var(--code-selection)" },
    ".cm-activeLine": { backgroundColor: "var(--code-line)" },
    ".cm-gutters": {
      backgroundColor: "var(--code-bg)",
      color: "var(--code-gutter-fg)",
      borderRight: "1px solid var(--code-border)",
    },
    ".cm-activeLineGutter": { backgroundColor: "var(--code-line)", color: "var(--code-fg)" },
    ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
      backgroundColor: "var(--code-bracket)",
      outline: "none",
    },
    ".cm-searchMatch": { backgroundColor: "var(--code-selection)" },
  });

  const highlight = HighlightStyle.define([
    { tag: [tags.lineComment, tags.blockComment, tags.docComment], color: "var(--code-comment)", fontStyle: "italic" },
    {
      tag: [tags.keyword, tags.moduleKeyword, tags.definitionKeyword, tags.operatorKeyword, tags.modifier],
      color: "var(--code-kw)",
    },
    { tag: tags.controlKeyword, color: "var(--code-kw2)" },
    { tag: [tags.string, tags.special(tags.string)], color: "var(--code-str)" },
    { tag: tags.escape, color: "var(--code-esc)" },
    { tag: tags.regexp, color: "var(--code-re)" },
    { tag: [tags.number, tags.bool, tags.null, tags.atom], color: "var(--code-num)" },
    { tag: [tags.constant(tags.name), tags.standard(tags.name)], color: "var(--code-const)" },
    {
      tag: [tags.typeName, tags.className, tags.namespace, tags.definition(tags.typeName), tags.definition(tags.className)],
      color: "var(--code-type)",
    },
    {
      tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.macroName],
      color: "var(--code-func)",
    },
    { tag: [tags.variableName, tags.propertyName, tags.labelName], color: "var(--code-var)" },
    {
      tag: [
        tags.operator,
        tags.derefOperator,
        tags.arithmeticOperator,
        tags.logicOperator,
        tags.bitwiseOperator,
        tags.compareOperator,
        tags.updateOperator,
        tags.definitionOperator,
        tags.typeOperator,
        tags.controlOperator,
      ],
      color: "var(--code-op)",
    },
    {
      tag: [tags.punctuation, tags.separator, tags.bracket, tags.angleBracket, tags.squareBracket, tags.paren, tags.brace],
      color: "var(--code-punct)",
    },
    { tag: [tags.meta, tags.annotation, tags.processingInstruction], color: "var(--code-meta)" },
    { tag: [tags.tagName, tags.angleBracket], color: "var(--code-tag)" },
    { tag: tags.attributeName, color: "var(--code-attr)" },
    { tag: tags.attributeValue, color: "var(--code-str)" },
    { tag: tags.self, color: "var(--code-kw2)" },
    { tag: tags.invalid, color: "var(--code-invalid)" },
    { tag: [tags.link, tags.url], color: "var(--code-link)" },
    { tag: tags.heading, color: "var(--code-heading)", fontWeight: "bold" },
  ]);

  return [chrome, syntaxHighlighting(highlight)];
}
