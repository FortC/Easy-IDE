<template>
  <div ref="wrapEl" class="se-wrap">
    <div ref="host" class="cm-host" :class="{ 'is-code': !isMd }" />
    <!-- 选中文本时浮动的 AI 入口 -->
    <button
      v-show="aiBtn.visible"
      class="se-ai-btn"
      :style="{ left: aiBtn.x + 'px', top: aiBtn.y + 'px' }"
      :title="t('se.askAi')"
      @mousedown.prevent
      @click="askAi"
    >
      AI
    </button>

    <!-- Alt+点击同名多命中时的定义选择器 -->
    <div
      v-if="pick.open"
      class="gd-pick"
      :style="{ left: pickPos.x + 'px', top: pickPos.y + 'px' }"
      @mousedown.stop
    >
      <div class="gd-pick-hd">
        <span class="gd-pick-word">{{ pick.word }}</span>
        <span class="gd-pick-path">{{ pick.list.length }}</span>
      </div>
      <button
        v-for="it in pick.list"
        :key="it.path + ':' + it.line"
        class="gd-pick-row"
        :title="it.path + ':' + it.line"
        @click="gotoTarget(it)"
      >
        <span class="gd-pick-kind">{{ kindTag(it.kind) }}</span>
        <span class="gd-pick-main">
          <span class="gd-pick-name">{{ it.name }}</span>
          <span v-if="it.container" class="gd-pick-ct">{{ it.container }}</span>
        </span>
        <span class="gd-pick-path">{{ it.path.split('/').slice(-2).join('/') }}:{{ it.line }}</span>
      </button>
    </div>

    <!-- 找不到定义时的瞬时提示（过渡复用 anim.css 的 emd-fade，不重复定义） -->
    <Transition name="emd-fade">
      <div v-if="flash.on" class="gd-flash">{{ flash.msg }}</div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref, watch, computed } from "vue";
import { EditorState, RangeSetBuilder, Compartment, StateEffect, StateField, type Extension } from "@codemirror/state";
import {
  EditorView,
  Decoration,
  type DecorationSet,
  ViewPlugin,
  keymap,
  lineNumbers,
  highlightActiveLine,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { autocompletion, closeBrackets, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import {
  syntaxHighlighting,
  defaultHighlightStyle,
  bracketMatching,
  foldGutter,
  foldKeymap,
  indentOnInput,
  indentUnit,
} from "@codemirror/language";
import { api } from "../../ipc/tauri";
import { useEditorStore } from "../../stores/editor";
import { useNotesIndexStore } from "../../stores/notesIndex";
import { useAgentStore } from "../../stores/agent";
import { useSettingsStore } from "../../stores/settings";
import { codeLangOf, isMarkdown } from "../../lib/filetypes";
import { applyCodeThemeVars, codeThemeExtensions, resolveCodeTheme } from "../../lib/editorThemes";
import { t, tf } from "../../i18n";
import { resolveDefinition, wordAtPos, type GotoTarget } from "../../lib/gotoDef";
import { suggestSymbols } from "../../lib/localSuggest";
import type { SymbolHit } from "../../types";

const host = ref<HTMLElement>();
const wrapEl = ref<HTMLElement>();
const editor = useEditorStore();
const indexStore = useNotesIndexStore();
const agentStore = useAgentStore();
const settings = useSettingsStore();
let view: EditorView | null = null;
/** 选区同步函数的转发引用（makeState 的 updateListener 需要提前访问） */
let syncSelectionRef: (() => void) | null = null;
let syncing = false; // 区分"编辑器输入"与"外部写入"

/** 当前活动文档是否为 markdown（否则按代码文件渲染） */
const isMd = computed(() => isMarkdown(editor.activePath));

/** 语言扩展舱：代码文件的语言包按需异步加载后注入 */
const langComp = new Compartment();

// ---------- 选中浮动 AI 按钮 ----------
const aiBtn = reactive({ visible: false, x: 0, y: 0, text: "" });

function updateAiButton() {
  const v = view;
  if (!v || !wrapEl.value) return;
  const sel = v.state.selection.main;
  const text = v.state.sliceDoc(sel.from, sel.to);
  if (sel.empty || !text.trim()) {
    aiBtn.visible = false;
    return;
  }
  aiBtn.text = text;
  const coords = v.coordsAtPos(sel.head);
  const rect = wrapEl.value.getBoundingClientRect();
  if (!coords) {
    aiBtn.visible = false;
    return;
  }
  aiBtn.x = Math.min(Math.max(coords.left - rect.left, 8), (rect.width || 400) - 60);
  aiBtn.y = Math.max(coords.top - rect.top - 32, 6);
  aiBtn.visible = true;
}

function askAi() {
  aiBtn.visible = false;
  // 导向 AI 面板；选区上下文已由 syncSelection 挂载，直接问即可
  agentStore.openPanel();
}

// ---------- 格式动作（由头部工具栏通过事件触发） ----------
function runAction(key: string) {
  const v = view;
  if (!v) return;
  const map: Record<string, () => void> = {
    h1: () => toggleHeading(v, 1),
    h2: () => toggleHeading(v, 2),
    h3: () => toggleHeading(v, 3),
    bold: () => wrapSel(v, "**"),
    italic: () => wrapSel(v, "*"),
    strike: () => wrapSel(v, "~~"),
    icode: () => wrapSel(v, "`"),
    mark: () => wrapSel(v, "==", "==", t("se.phMark")),
    link: () => insertLink(v, false),
    wikilink: () => wrapSel(v, "[[", "]]", t("se.phNote")),
    image: () => insertLink(v, true),
    ul: () => toggleLinePrefix(v, "- "),
    ol: () => toggleLinePrefix(v, "1. "),
    task: () => toggleLinePrefix(v, "- [ ] "),
    quote: () => toggleLinePrefix(v, "> "),
    table: () => insertBlock(v, t("se.table")),
    hr: () => insertBlock(v, "---"),
    taskdone: () => toggleTaskDone(v),
    indent: () => indentLines(v, 1),
    outdent: () => indentLines(v, -1),
    clearfmt: () => clearFormat(v),
  };
  map[key]?.();
  v.focus();
}

function onFormatEvent(e: Event) {
  runAction((e as CustomEvent<string>).detail);
}

function onInsertText(e: Event) {
  const v = view;
  if (!v) return;
  const text = (e as CustomEvent<string>).detail;
  const sel = v.state.selection.main;
  v.dispatch({
    changes: { from: sel.from, to: sel.to, insert: text },
    selection: { anchor: sel.from + text.length },
    effects: EditorView.scrollIntoView(sel.from + text.length),
  });
  v.focus();
}

/** AI 结果替换当前选区 */
function onReplaceSelection(e: Event) {
  const v = view;
  if (!v) return;
  const text = (e as CustomEvent<string>).detail;
  const sel = v.state.selection.main;
  const from = sel.empty ? sel.from : sel.from;
  const to = sel.empty ? sel.from : sel.to;
  v.dispatch({
    changes: { from, to, insert: text },
    selection: { anchor: from + text.length },
    effects: EditorView.scrollIntoView(from + text.length),
  });
  v.focus();
}

/** 任务行勾选切换：- [ ] ↔ - [x] */
function toggleTaskDone(v: EditorView) {
  const { state } = v;
  const sel = state.selection.main;
  const first = state.doc.lineAt(sel.from).number;
  const last = state.doc.lineAt(sel.to).number;
  const changes: { from: number; to?: number; insert: string }[] = [];
  const re = /^(\s*(?:[-*+]|\d+\.)\s+\[)( |x|X)(\])/;
  for (let n = first; n <= last; n++) {
    const line = state.doc.line(n);
    const m = re.exec(line.text);
    if (m) {
      const from = line.from + m[1].length;
      changes.push({ from, to: from + 1, insert: m[2] === " " ? "x" : " " });
    }
  }
  if (changes.length === 0) {
    // 没有任务标记则把当前行变成已勾选任务
    changes.push({ from: state.doc.line(first).from, insert: "- [x] " });
  }
  v.dispatch({ changes });
}

/** 缩进 / 减少 缩进（2 空格） */
function indentLines(v: EditorView, dir: 1 | -1) {
  const { state } = v;
  const sel = state.selection.main;
  const first = state.doc.lineAt(sel.from).number;
  const last = state.doc.lineAt(sel.to).number;
  const changes: { from: number; to?: number; insert: string }[] = [];
  for (let n = first; n <= last; n++) {
    const line = state.doc.line(n);
    if (dir === 1) {
      changes.push({ from: line.from, insert: "  " });
    } else if (line.text.startsWith("  ")) {
      changes.push({ from: line.from, to: line.from + 2, insert: "" });
    } else if (line.text.startsWith(" ") || line.text.startsWith("\t")) {
      changes.push({ from: line.from, to: line.from + 1, insert: "" });
    }
  }
  v.dispatch({ changes });
}

/** 清除选区内常见 markdown 标记 */
function clearFormat(v: EditorView) {
  const { state } = v;
  const sel = state.selection.main;
  let text = state.sliceDoc(sel.from, sel.to);
  text = text
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/~~([^~]+)~~/g, "$1")
    .replace(/==([^=]+)==/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^>\s?/gm, "")
    .replace(/^\s*[-*+]\s+(\[[ xX]\]\s+)?/gm, "")
    .replace(/^\s*\d+\.\s+/gm, "");
  v.dispatch({
    changes: { from: sel.from, to: sel.to, insert: text },
    selection: { anchor: sel.from + text.length },
  });
}

/** 选区包裹/解包（再次点击取消） */
function wrapSel(v: EditorView, before: string, after = before, placeholder = t("se.phText")) {
  const { state } = v;
  const sel = state.selection.main;
  const selected = state.sliceDoc(sel.from, sel.to);
  const hasEmpty = selected.length === 0;
  const text = hasEmpty ? placeholder : selected;

  const beforeCtx = state.sliceDoc(Math.max(0, sel.from - before.length), sel.from);
  const afterCtx = state.sliceDoc(sel.to, Math.min(state.doc.length, sel.to + after.length));
  if (beforeCtx === before && afterCtx === after && !hasEmpty) {
    // 解包
    v.dispatch({
      changes: [
        { from: sel.from - before.length, to: sel.from, insert: "" },
        { from: sel.to, to: sel.to + after.length, insert: "" },
      ],
      selection: { anchor: sel.from - before.length },
    });
    return;
  }
  v.dispatch({
    changes: { from: sel.from, to: sel.to, insert: before + text + after },
    selection: {
      anchor: sel.from + before.length,
      head: sel.from + before.length + text.length,
    },
  });
}

/** 链接/图片：插入后选中占位文字，直接输入替换 */
function insertLink(v: EditorView, isImage: boolean) {
  const { state } = v;
  const sel = state.selection.main;
  const selected = state.sliceDoc(sel.from, sel.to);
  const alt = selected || (isImage ? t("se.phAlt") : t("se.phLink"));
  const url = isImage ? t("se.phImg") : "https://";
  const insert = `${isImage ? "!" : ""}[${alt}](${url})`;
  v.dispatch({
    changes: { from: sel.from, to: sel.to, insert },
    selection: { anchor: sel.from + (isImage ? 2 : 1), head: sel.from + (isImage ? 2 : 1) + alt.length },
  });
}

/** 行前缀切换：全部已带则去掉，否则加上 */
function toggleLinePrefix(v: EditorView, prefix: string) {
  const { state } = v;
  const sel = state.selection.main;
  const first = state.doc.lineAt(sel.from).number;
  const last = state.doc.lineAt(sel.to).number;
  const changes: { from: number; to?: number; insert: string }[] = [];
  let allHave = true;
  for (let n = first; n <= last; n++) {
    if (!state.doc.line(n).text.startsWith(prefix)) allHave = false;
  }
  for (let n = first; n <= last; n++) {
    const line = state.doc.line(n);
    if (allHave) {
      changes.push({ from: line.from, to: line.from + prefix.length, insert: "" });
    } else if (!line.text.startsWith(prefix)) {
      changes.push({ from: line.from, insert: prefix });
    }
  }
  v.dispatch({ changes });
}

/** 标题切换：其它级别替换为该级别，相同则取消 */
function toggleHeading(v: EditorView, level: number) {
  const prefix = "#".repeat(level) + " ";
  const { state } = v;
  const sel = state.selection.main;
  const first = state.doc.lineAt(sel.from).number;
  const last = state.doc.lineAt(sel.to).number;
  const changes: { from: number; to?: number; insert: string }[] = [];
  const re = /^#{1,6} /;
  for (let n = first; n <= last; n++) {
    const line = state.doc.line(n);
    const m = re.exec(line.text);
    if (m && m[0] === prefix) {
      changes.push({ from: line.from, to: line.from + prefix.length, insert: "" });
    } else if (m) {
      changes.push({ from: line.from, to: line.from + m[0].length, insert: prefix });
    } else {
      changes.push({ from: line.from, insert: prefix });
    }
  }
  v.dispatch({ changes });
}

/** 块级插入（表格/分隔线）：保证前后空行 */
function insertBlock(v: EditorView, text: string) {
  const { state } = v;
  const sel = state.selection.main;
  const atLineStart = state.doc.lineAt(sel.from).from === sel.from;
  const prefix = atLineStart ? "" : "\n\n";
  v.dispatch({
    changes: { from: sel.from, to: sel.to, insert: prefix + text + "\n" },
    selection: { anchor: sel.from + prefix.length + text.length + 1 },
  });
}

// ---- [[ ]] / #tag / ==高亮== 高亮 ----
const reMark = /!?\[\[[^\[\]\n]+\]\]|#[\p{L}\p{N}_][\p{L}\p{N}_/-]*|==[^=\n]+==/gu;

function buildDeco(v: EditorView): DecorationSet {
  const b = new RangeSetBuilder<Decoration>();
  for (const { from, to } of v.visibleRanges) {
    const text = v.state.doc.sliceString(from, to);
    let m: RegExpExecArray | null;
    reMark.lastIndex = 0;
    while ((m = reMark.exec(text))) {
      const s = from + m.index;
      const t = m[0];
      let deco: Decoration;
      if (t.startsWith("![[")) deco = Decoration.mark({ class: "cm-embed-mark" });
      else if (t.startsWith("[[")) deco = Decoration.mark({ class: "cm-wikilink" });
      else if (t.startsWith("==")) deco = Decoration.mark({ class: "cm-mark" });
      else deco = Decoration.mark({ class: "cm-mdtag" });
      b.add(s, s + t.length, deco);
    }
  }
  return b.finish();
}

const markPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(v: EditorView) {
      this.decorations = buildDeco(v);
    }
    update(u: { docChanged: boolean; viewportChanged: boolean; view: EditorView }) {
      if (u.docChanged || u.viewportChanged) this.decorations = buildDeco(u.view);
    }
  },
  { decorations: (v) => v.decorations },
);

// ---- 补全：[[ 笔记名 / # 标签 ----
function noteCompletion(ctx: CompletionContext) {
  const before = ctx.state.doc.sliceString(Math.max(0, ctx.pos - 2), ctx.pos);
  if (before !== "[[") return null;
  return {
    from: ctx.pos,
    options: indexStore.linkSuggestions.map((s) => ({
      label: s.label,
      apply: s.label + "]]",
      detail: s.path,
      type: "text",
    })),
    validFor: /^[^\[\]]*$/,
  };
}

function tagCompletion(ctx: CompletionContext) {
  const word = ctx.matchBefore(/#[\p{L}\p{N}_/-]+/u);
  if (!word) return null;
  if (word.from === word.to && !ctx.explicit) return null;
  return {
    from: word.from,
    options: indexStore.tags.map((t) => ({
      label: "#" + t.tag,
      detail: tf("se.nNotes", { n: t.count }),
      type: "text",
    })),
    validFor: /^#[\p{L}\p{N}_/-]*$/u,
  };
}

// ---- B7-A 档：本地符号联想 ----
// 返回 Promise（CompletionResult | Promise<CompletionResult | null> 是 API 允许的），
// 联想走 IPC 查符号索引，所以必须异步；ctx.explicit 表示 Ctrl+Space 强触发。
let symbolCompletionSeq = 0;
function symbolCompletion(ctx: CompletionContext) {
  const word = ctx.matchBefore(/[\p{L}\p{N}_$][\p{L}\p{N}_$]*/u);
  if (!word) return null;
  if (word.from === word.to && !ctx.explicit) return null;
  // 字符串字面量里不做联想：补全会打断正常输入，且几乎从不正确
  if (insideStringOrComment(ctx)) return null;
  const prefix = ctx.state.sliceDoc(word.from, word.to);
  const seq = ++symbolCompletionSeq;
  const path = editor.activePath;
  return suggestSymbols(prefix, path).then((items) => {
    // 用户继续打字后又发起了新查询，丢弃这次过期结果，避免列表闪回旧内容
    if (seq !== symbolCompletionSeq || !items.length) return null;
    return {
      from: word.from,
      options: items.map((s) => ({
        label: s.label,
        detail: s.detail,
        type: "variable",
        // kind 已体现在 detail 里，不重复占一行
        info: s.kind,
      })),
      validFor: /^[\p{L}\p{N}_$]*$/u,
    };
  });
}

/** 粗判光标是否位于字符串/注释内：语言包未就绪时也保守跳过，宁可少联想不可错联想 */
function insideStringOrComment(ctx: CompletionContext): boolean {
  const line = ctx.state.doc.lineAt(ctx.pos);
  const text = line.text.slice(0, ctx.pos - line.from);
  // 未闭合的引号 / 块注释起始
  const quote = (text.match(/"/g) || []).length + (text.match(/'/g) || []).length;
  if (quote % 2 === 1) return true;
  if (/^\s*(\/\/|#)/.test(text)) return true;
  return /\/\*(?!(?:.|\n)*?\*\/)/.test(text) && !/\*\//.test(text);
}

// ---- 粘贴图片 → 附件目录 ----
const pasteHandler = EditorView.domEventHandlers({
  paste(event, v) {
    const items = event.clipboardData?.items;
    if (!items) return false;
    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        const file = item.getAsFile();
        if (!file) continue;
        event.preventDefault();
        const ext = (file.name.split(".").pop() || "png").toLowerCase();
        file.arrayBuffer().then(async (buf) => {
          const rel = await api.saveImage(Array.from(new Uint8Array(buf)), ext);
          v.dispatch({
            changes: {
              from: v.state.selection.main.head,
              insert: `![](${rel})`,
            },
          });
        });
        return true;
      }
    }
    return false;
  },
});

// ---- Alt+点击跳定义（B5.1） ----
/** 同名多命中时的选择器；单命中直接跳，不打扰用户 */
const pick = reactive<{ open: boolean; word: string; list: GotoTarget[] }>({
  open: false,
  word: "",
  list: [],
});
/** Alt 按下时高亮光标下的标识符，给出「可跳转」的视觉暗示 */
const altHint = reactive<{ on: boolean; from: number; to: number }>({
  on: false,
  from: 0,
  to: 0,
});
/** 正在解析中：避免 Alt 连点打爆后端 */
let defBusy = false;

function closePick() {
  pick.open = false;
  pick.list = [];
  pick.word = "";
}

/** 跳到目标位置；跨文件时走 openNote + pendingJump（同 RefsPanel 已验证的模式） */
function gotoTarget(tg: GotoTarget) {
  const cur = editor.activePath;
  closePick();
  if (tg.path === cur) {
    gotoLine(Math.max(0, tg.line - 1));
    return;
  }
  // openNote 会保存当前编辑内容并切 activePath，pendingJump 由 watch 消费
  editor.pendingJump = { line: Math.max(0, tg.line - 1) };
  void editor.openNote(tg.path);
}

async function doGoto(word: string, anchor?: number) {
  if (defBusy || !word) return;
  defBusy = true;
  try {
    const list = await resolveDefinition(word, editor.activePath);
    if (!list.length) {
      flashHint(t("gd.notFound"));
      return;
    }
    if (list.length === 1) gotoTarget(list[0]);
    else {
      pick.word = word;
      pick.list = list;
      pick.open = true;
      // 落在光标处，比鼠标位置更稳定（mousedown 已把光标移到点击点）
      if (view && anchor !== undefined) {
        const c = view.coordsAtPos(anchor);
        if (c) placePick(c.left, c.bottom);
        else placePick(0, 0);
      }
    }
  } finally {
    defBusy = false;
  }
}

/** 顶部瞬时提示（找不到定义时用，220ms 自动消失） */
const flash = reactive({ msg: "", on: false });
let flashTimer: number | undefined;
function flashHint(msg: string) {
  flash.msg = msg;
  flash.on = true;
  if (flashTimer) window.clearTimeout(flashTimer);
  flashTimer = window.setTimeout(() => (flash.on = false), 2200);
}

function kindTag(kind: string): string {
  return (
    {
      class: "C",
      interface: "I",
      enum: "E",
      record: "R",
      method: "M",
      function: "F",
      field: "A",
    }[kind] ?? "?"
  );
}

/** 选择器定位：跟随 Alt 提示位置，夹在编辑器可视区内 */
const pickPos = reactive({ x: 0, y: 0 });
function placePick(clientX: number, clientY: number) {
  const wrap = wrapEl.value;
  if (!wrap) return;
  const r = wrap.getBoundingClientRect();
  const w = 320;
  const h = Math.min(pick.list.length * 26 + 12, 300);
  pickPos.x = Math.min(Math.max(8, clientX - r.left), Math.max(8, r.width - w - 8));
  pickPos.y = Math.min(Math.max(8, clientY - r.top + 18), Math.max(8, r.height - h - 8));
}

/**
 * Alt+点击：取光标处标识符并跳转。
 * 只在代码语境生效（md 下 Alt 是无害的，但符号索引对 md 无意义，故不启用）。
 */
const gotoDefHandler = EditorView.domEventHandlers({
  mousedown(event, v) {
    if (!event.altKey || event.button !== 0 || isMd.value) return false;
    const pos = v.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos === null || pos === undefined) return false;
    const word = wordAtPos(v.state.doc.toString(), pos);
    if (!word) return false;
    event.preventDefault();
    event.stopPropagation();
    closePick();
    void doGoto(word, pos);
    return true;
  },
  mousemove(event, v) {
    if (isMd.value) return false;
    const alt = event.altKey;
    if (!alt) {
      if (altHint.on) {
        altHint.on = false;
        v.dispatch({ effects: hintEffect(null) });
      }
      return false;
    }
    const pos = v.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos === null || pos === undefined) {
      altHint.on = false;
      return false;
    }
    const text = v.state.doc.toString();
    const word = wordAtPos(text, pos);
    if (!word) {
      altHint.on = false;
      return false;
    }
    // 用行内偏移反推 from/to（wordAtPos 只返回词，不返回区间）
    const line = v.state.doc.lineAt(pos);
    const rel = text.indexOf(word, line.from);
    const from = rel >= 0 && rel + word.length <= line.to ? rel : line.from;
    altHint.on = true;
    altHint.from = from;
    altHint.to = from + word.length;
    v.dispatch({ effects: hintEffect(Decoration.set(hintMark.range(from, from + word.length))) });
    return false;
  },
});

/** Alt 悬停高亮：用 StateEffect 驱动，避免重建 editor */
const setHint = StateEffect.define<DecorationSet>();
const hintMark = Decoration.mark({ class: "cm-goto-def-hint" });
const hintState = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    for (const e of tr.effects) if (e.is(setHint)) return e.value;
    // 文档变化后旧区间失效，必须跟随映射，否则高亮会漂移到别处
    return deco.map(tr.changes);
  },
  provide: (f) => EditorView.decorations.from(f),
});
const hintEffect = (v: DecorationSet | null) => setHint.of(v ?? Decoration.none);

function makeState(): EditorState {
  const md = isMd.value;
  const exts: Extension[] = [
    history(),
    // AI5：选区变化时把选中的代码挂到 agent store（Ctrl+I 唤起 AI 直接可用）
    EditorView.updateListener.of(() => syncSelectionRef?.()),
    keymap.of([
      // Ctrl+B 加粗 / Ctrl+I 斜体（Obsidian 习惯，仅 md 生效）
      { key: "Mod-b", run: () => (view ? (wrapSel(view, "**"), true) : false) },
      { key: "Mod-i", run: () => (view ? (wrapSel(view, "*"), true) : false) },
      ...defaultKeymap,
      ...historyKeymap,
    ]),
    // md 保持默认高亮 + UI 令牌；代码文件走可切换配色主题（--code-* 变量驱动）
    ...(md ? [syntaxHighlighting(defaultHighlightStyle)] : codeThemeExtensions()),
    langComp.of([]),
    EditorView.updateListener.of((u) => {
      if (u.docChanged && !syncing) {
        syncing = true;
        editor.setContent(u.state.doc.toString());
        syncing = false;
      }
      if (u.selectionSet || u.docChanged) updateAiButton();
    }),
  ];
  if (md) {
    exts.push(
      markdown(),
      markPlugin,
      autocompletion({
        override: [
          (ctx: CompletionContext): CompletionResult | null => noteCompletion(ctx) ?? tagCompletion(ctx),
        ],
      }),
      pasteHandler,
      EditorView.lineWrapping,
    );
  } else {
    // 代码文件：行号 + 折叠 + 括号匹配 + 自动补括号；不折行（横向滚动）
    exts.push(
      lineNumbers(),
      foldGutter(),
      indentOnInput(),
      indentUnit.of("  "),
      bracketMatching(),
      closeBrackets(),
      highlightActiveLine(),
      // Alt+点击跳定义 + Alt 悬停高亮（仅代码语境，md 无符号索引）
      hintState,
      gotoDefHandler,
      // B7-A 档：本地符号联想（复用 code_index，零依赖零模型不联网）
      autocompletion({
        override: [symbolCompletion],
        activateOnTyping: true,
        // 打字时不因「已高亮」而吞掉补全请求，否则联想很难主动弹出
        icons: false,
      }),
      keymap.of([...foldKeymap]),
    );
  }
  exts.push(
    EditorView.theme({
      "&": {
        height: "100%",
        fontSize: md ? "var(--font-text-size)" : "calc(13px * var(--font-scale))",
      },
      ".cm-scroller": {
        fontFamily: md ? "var(--font-text)" : "var(--font-mono)",
        lineHeight: md ? "1.6" : "1.55",
        padding: md ? "12px 8px" : "8px 0",
      },
      ".cm-content": { caretColor: "var(--interactive-accent)" },
      "&.cm-focused": { outline: "none" },
    }),
  );
  return EditorState.create({ doc: editor.content, extensions: exts });
}

/** 代码文件的语言包按需异步加载，就绪后注入语言舱 */
function loadLanguage() {
  const spec = codeLangOf(editor.activePath);
  if (!spec) return;
  spec
    .load()
    .then((exts) => {
      view?.dispatch({ effects: langComp.reconfigure(exts) });
    })
    .catch(() => {
      /* 语言包加载失败则保持纯文本编辑 */
    });
}

/**
 * 同步代码配色主题变量到宿主元素。
 * 高亮样式与 chrome 都引用 --code-* 变量，换肤只需重设变量（零重建）；
 * md 模式清除变量，回落 UI 令牌。
 */
function syncThemeVars() {
  const el = host.value;
  if (!el) return;
  if (isMd.value) {
    applyCodeThemeVars(el, null);
    return;
  }
  applyCodeThemeVars(el, resolveCodeTheme(settings.data.code_theme, settings.data.theme));
}

/** 定位到某行（0 基）并滚动到可视区顶部 */
function gotoLine(line: number) {
  if (!view) return;
  const l = view.state.doc.line(Math.min(Math.max(1, line + 1), view.state.doc.lines));
  view.dispatch({
    selection: { anchor: l.from },
    effects: EditorView.scrollIntoView(l.from, { y: "start" }),
  });
  view.focus();
}

onMounted(() => {
  if (!host.value) return;
  syncThemeVars();
  view = new EditorView({ parent: host.value, state: makeState() });

  // AI5 选区上下文：选中代码即挂载到 agent store，
  // Ctrl+I 唤起 AI 面板即可直接提问，无需复制粘贴
  const syncSelection = () => {
    const v = view;
    if (!v) return;
    const sel = v.state.selection.main;
    const path = editor.activePath;
    if (sel.empty || !path) {
      agentStore.setSelectionContext(null);
      return;
    }
    const text = v.state.sliceDoc(sel.from, sel.to);
    if (!text.trim()) {
      agentStore.setSelectionContext(null);
      return;
    }
    const lineFrom = v.state.doc.lineAt(sel.from);
    const lineTo = v.state.doc.lineAt(sel.to);
    agentStore.setSelectionContext({
      name: path.split("/").pop() ?? path,
      path,
      text,
      from: lineFrom.number - 1,
      to: lineTo.number - 1,
    });
  };

  // Ctrl+I 唤起 AI 面板（选中代码已自动挂载为上下文）。
  // 注意：md 模式下 Ctrl+i 已被斜体占用，故仅代码模式生效。
  const onAskAi = (e: KeyboardEvent) => {
    if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== "i") return;
    if (!view || !view.hasFocus || isMd.value) return;
    e.preventDefault();
    agentStore.openPanel();
  };
  window.addEventListener("keydown", onAskAi);
  // Alt+点击后的消歧：Esc 关选择器，数字键直选（≤9 项）
  const onPickKey = (e: KeyboardEvent) => {
    if (!pick.open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      closePick();
      return;
    }
    if (/^[1-9]$/.test(e.key)) {
      const it = pick.list[Number(e.key) - 1];
      if (it) {
        e.preventDefault();
        gotoTarget(it);
      }
    }
  };
  window.addEventListener("keydown", onPickKey);
  onUnmounted(() => {
    window.removeEventListener("keydown", onAskAi);
    window.removeEventListener("keydown", onPickKey);
  });

  syncSelectionRef = syncSelection;
  // 初始化时同步一次（可能已有选区）
  syncSelection();

  // 容器尺寸变化（拖窗口/拖分屏分隔条）后强制重新测量，避免留白
  const ro = new ResizeObserver(() => view?.requestMeasure());
  ro.observe(host.value);
  onUnmounted(() => ro.disconnect());

  // 大纲/搜索跳转到某行
  const onGoto = (e: Event) => gotoLine((e as CustomEvent<number>).detail);
  window.addEventListener("emd-goto-line", onGoto);
  window.addEventListener("emd-format", onFormatEvent);
  window.addEventListener("emd-insert-text", onInsertText);
  window.addEventListener("emd-replace-selection", onReplaceSelection);
  onUnmounted(() => {
    window.removeEventListener("emd-goto-line", onGoto);
    window.removeEventListener("emd-format", onFormatEvent);
    window.removeEventListener("emd-insert-text", onInsertText);
    window.removeEventListener("emd-replace-selection", onReplaceSelection);
  });
});

onUnmounted(() => {
  view?.destroy();
  view = null;
});

// 切换文件 → 重建文档
watch(
  () => editor.openToken,
  () => {
    view?.setState(makeState());
    // 代码文件：异步加载语言包
    loadLanguage();
    // 文件类型可能变化（md ↔ 代码），同步配色主题变量
    syncThemeVars();
    // 反链等行号跳转：纯源码模式没有预览组件兜底，这里直接定位；
    // 有锚点时留给预览组件处理
    const jump = editor.pendingJump;
    if (jump?.line !== undefined) {
      editor.pendingJump = jump.anchor ? { anchor: jump.anchor } : null;
      gotoLine(jump.line);
    }
  },
);

// 外部写入（属性面板 / 模板插入）→ 同步到编辑器
watch(
  () => editor.content,
  (c) => {
    if (!view || syncing) return;
    if (view.state.doc.toString() !== c) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: c } });
    }
  },
);

// 代码配色主题 / 界面主题（auto 跟随）变化 → 即时换肤（只换变量，不重建编辑器）
watch(
  () => [settings.data.code_theme, settings.data.theme],
  () => syncThemeVars(),
);
</script>

<style scoped>
.se-wrap {
  position: relative;
  height: 100%;
  width: 100%;
  min-height: 0;
}
.se-ai-btn {
  position: absolute;
  z-index: 30;
  height: 22px;
  padding: 0 9px;
  border-radius: var(--radius-m);
  background: var(--interactive-accent);
  color: var(--text-on-accent);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.03em;
  box-shadow: var(--shadow-l1);
}
.se-ai-btn:hover {
  background: var(--interactive-accent-hover);
}
/* 高频交互必须给按压反馈；scoped 组件要自己写（anim.css 全局选择器命不中） */
.se-ai-btn:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .se-ai-btn:active {
    transform: none;
  }
}

/* ---- Alt+点击跳定义：选择器 ---- */
.gd-pick {
  position: absolute;
  z-index: 40;
  width: 300px;
  max-width: 90%;
  padding: 4px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  background: var(--background-secondary);
  box-shadow: var(--shadow-l2);
  animation: gd-in var(--anim-enter);
}
@keyframes gd-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}
/* 头/名/容器共用省略号 */
.gd-pick-word,
.gd-pick-name,
.gd-pick-ct {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gd-pick-hd {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px 6px;
  border-bottom: 1px solid var(--background-modifier-border);
}
.gd-pick-word {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-normal);
}
.gd-pick-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 4px 6px;
  border: 0;
  border-radius: var(--radius-s);
  background: transparent;
  color: var(--text-normal);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  transition: background var(--anim-hover);
}
.gd-pick-row:hover {
  background: var(--background-modifier-hover);
}
/* scoped 组件必须自带按压反馈（anim.css 全局选择器命不中） */
.gd-pick-row:active {
  transform: scale(0.98);
}
@media (hover: none) {
  .gd-pick-row:active {
    transform: none;
  }
}
.gd-pick-kind {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  display: grid;
  place-items: center;
  border-radius: var(--radius-s);
  background: var(--interactive-accent-hover-alt);
  color: var(--text-accent);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
}
.gd-pick-main {
  display: flex;
  align-items: baseline;
  gap: 5px;
  min-width: 0;
}
.gd-pick-name {
  font-family: var(--font-mono);
}
.gd-pick-ct,
.gd-pick-path {
  font-size: 10px;
  color: var(--text-faint);
}
.gd-pick-path {
  margin-left: auto;
  flex-shrink: 0;
}
/* 找不到定义时的瞬时提示（过渡复用 anim.css 的 .emd-fade-*） */
.gd-flash {
  position: absolute;
  top: 10px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 45;
  padding: 5px 12px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  background: var(--background-secondary);
  color: var(--text-muted);
  font-size: 12px;
  box-shadow: var(--shadow-l1);
  pointer-events: none;
}
</style>

<style>
.cm-host {
  height: 100%;
  width: 100%;
  min-height: 0;
  overflow: hidden;
}
.cm-host .cm-editor {
  height: 100%;
}
.cm-host .cm-scroller {
  overflow-y: auto;
  /* 代码配色主题变量优先（lib/editorThemes.ts），未设置时回落 UI 令牌（md 等场景） */
  background: var(--code-bg, var(--background-primary));
  color: var(--code-fg, var(--text-normal));
}
.cm-host .cm-gutters {
  display: none;
}
/* Alt 按下时，光标下标识符提示「可跳转」（仅描边，不动 color 以免与主题高亮打架） */
.cm-host .cm-goto-def-hint {
  text-decoration: underline;
  text-decoration-style: dotted;
  text-decoration-color: var(--interactive-accent);
  text-underline-offset: 3px;
  cursor: pointer;
}
/* 代码文件：显示行号/折叠槽 */
.cm-host.is-code .cm-gutters {
  display: block;
  background: var(--code-bg, var(--background-primary));
  border-right: 1px solid var(--code-border, var(--background-modifier-border));
  color: var(--code-gutter-fg, var(--text-faint));
  font-family: var(--font-mono);
  font-size: calc(12px * var(--font-scale));
  min-width: 44px;
}
.cm-host.is-code .cm-activeLineGutter {
  background: var(--code-line, var(--background-primary-alt));
  color: var(--code-fg, var(--text-muted));
}
.cm-host.is-code .cm-foldGutter span {
  color: var(--text-faint);
}
.cm-host.is-code .cm-foldGutter span:hover {
  color: var(--text-normal);
}
.cm-host .cm-activeLine {
  background: var(--code-line, var(--background-primary-alt));
}
/* wikilink / embed / tag 着色（Obsidian 风格） */
.cm-wikilink {
  color: var(--link-color);
}
.cm-embed-mark {
  color: var(--text-accent);
}
.cm-mdtag {
  color: var(--tag-color);
  background: var(--tag-background);
  border-radius: var(--radius-s);
  padding: 0 2px;
}
.cm-mark {
  background: rgba(255, 208, 0, 0.28);
  border-radius: var(--radius-s);
  padding: 0 2px;
}
.cm-host .cm-selectionBackground,
.cm-host .cm-content ::selection {
  background: var(--code-selection, var(--selection-background)) !important;
}
.cm-host .cm-tooltip-autocomplete > ul {
  font-family: var(--font-ui);
  max-height: 240px;
}
</style>
