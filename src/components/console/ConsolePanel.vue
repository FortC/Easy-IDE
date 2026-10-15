<template>
  <div class="cs-panel" :class="{ 'is-collapsed': !ui.consoleVisible }" :style="{ height: height + 'px' }">
    <div
      class="cs-resizer"
      role="separator"
      aria-orientation="horizontal"
      :aria-valuenow="height"
      tabindex="0"
      @mousedown="startDrag"
      @dblclick="resetHeight"
      @keydown="onResizerKey"
    />
    <div class="cs-head">
      <div class="cs-tabs">
        <BaseTabs v-model="consoleTab" :items="consoleTabs" size="sm" aria-label="控制台视图" />
      </div>
      <span v-if="ui.consoleTab === 'output' && consoleStore.running.length > 0" class="cs-running">
        {{ tf("cs.running", { n: consoleStore.running.length }) }}
      </span>
      <div class="cs-actions">
        <template v-if="ui.consoleTab === 'output'">
          <button
            v-for="r in consoleStore.running"
            :key="r.id"
            class="emd-btn cs-kill"
            :title="t('cs.kill')"
            @click="consoleStore.kill(r.id)"
          >
            <Icon name="square" :size="11" /> {{ shortCmd(r.command) }}
          </button>
          <button class="cs-tool-btn" :title="t('cs.clear')" @click="consoleStore.clear()">
            <Icon name="eraser" :size="13" />
          </button>
        </template>
        <template v-else-if="ui.consoleTab === 'terminal'">
          <button v-if="termDead" class="emd-btn" :title="t('cs.termRestart')" @click="restartTerm">
            <Icon name="refresh-cw" :size="11" /> {{ t("cs.termRestart") }}
          </button>
          <button class="cs-tool-btn" :title="t('cs.kill')" @click="killTerm">
            <Icon name="square" :size="13" />
          </button>
        </template>
        <template v-else>
          <button class="cs-tool-btn" :title="t('cs.clear')" @click="consoleStore.clear()">
            <Icon name="eraser" :size="13" />
          </button>
        </template>
        <!-- 收起：仅降高度，结构保留（底部面板为常驻三页签结构） -->
        <button
          class="cs-tool-btn"
          :title="ui.consoleVisible ? t('cs.hide') : t('cs.show')"
          @click="ui.consoleVisible = !ui.consoleVisible"
        >
          <Icon :name="ui.consoleVisible ? 'chevron-down' : 'chevron-up'" :size="13" />
        </button>
      </div>
    </div>

    <!-- 输出视图：行数可能上万，走虚拟滚动只渲染可视区（见 lib/useVirtualList.ts） -->
    <div v-show="ui.consoleTab === 'output'" class="cs-body cs-virtual">
      <div v-if="consoleStore.runs.length === 0" class="cs-empty">{{ t("cs.empty") }}</div>
      <template v-else>
        <div ref="bodyEl" class="cs-vp" @scroll="onOutputScroll">
          <!-- 行高探针：不可见但参与布局，用于实测行高 -->
          <span ref="lineProbe" class="cs-line cs-probe" aria-hidden="true">0</span>
          <div class="cs-vp-inner" :style="{ height: vl.totalH.value + 'px' }">
            <div class="cs-vp-win" :style="{ transform: `translateY(${vl.offsetY.value}px)` }">
              <div
                v-for="row in visibleRows"
                :key="row.key"
                class="cs-line"
                :class="{ 'is-err': row.err, 'is-head': row.head }"
              >
                <template v-if="row.head">
                  <span class="cs-status" :class="'is-' + row.status">{{ statusLabel(row.status ?? 'running') }}</span>
                  <span class="cs-cmd">{{ row.text }}</span>
                  <span v-if="row.status !== 'running' && row.code !== null" class="cs-code">
                    exit {{ row.code }}
                  </span>
                </template>
                <template v-else>{{ row.text }}</template>
              </div>
            </div>
          </div>
        </div>
        <button
          v-show="atBottom"
          class="cs-jump-bottom"
          :title="t('cs.jumpBottom')"
          @click="jumpToBottom"
        >
          <Icon name="chevron-down" :size="13" />
        </button>
      </template>
    </div>

    <!-- 问题视图：从运行输出中抽取诊断行，点击定位到编辑器 -->
    <div v-show="ui.consoleTab === 'problems'" class="cs-body">
      <div v-if="problems.length === 0" class="cs-empty">{{ t("cs.noProblems") }}</div>
      <div v-for="(p, i) in problems" :key="i" class="cs-problem">
        <Icon name="alert-triangle" :size="12" class="cs-problem-icon" />
        <span class="cs-problem-loc" v-if="p.loc">{{ p.loc }}</span>
        <span class="cs-problem-msg">{{ p.msg }}</span>
      </div>
    </div>

    <!-- 终端视图 -->
    <div v-show="ui.consoleTab === 'terminal'" class="cs-term-body">
      <div ref="termHost" class="cs-term-host" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import Icon from "../common/Icon.vue";
import BaseTabs from "../common/BaseTabs.vue";
import { useUiStore } from "../../stores/ui";
import { useConsoleStore } from "../../stores/console";
import { api } from "../../ipc/tauri";
import { useVirtualList } from "../../lib/useVirtualList";
import type { RunStatus } from "../../stores/console";
import { t, tf } from "../../i18n";

const ui = useUiStore();
const consoleStore = useConsoleStore();
const height = ref(200);
const bodyEl = ref<HTMLElement>();
const termHost = ref<HTMLElement>();
const termDead = ref(false);

/** 控制台页签（问题页签仅代码语境出现） */
const consoleTab = computed({
  get: () => ui.consoleTab,
  set: (v: string) => (ui.consoleTab = v as typeof ui.consoleTab),
});
const consoleTabs = computed(() => {
  const items: { key: string; label: string; badge?: number }[] = [
    { key: "output", label: t("cs.tabOutput") },
    { key: "terminal", label: t("cs.tabTerminal") },
  ];
  if (ui.showProblemTab) {
    items.push({ key: "problems", label: t("cs.tabProblems"), badge: problems.value.length });
  }
  return items;
});

// xterm 实例（懒加载创建；类型仅用于引用）
let term: Terminal | null = null;
let fit: FitAddon | null = null;
let termId = 0;
let unOut: (() => void) | null = null;
let unExit: (() => void) | null = null;
let ro: ResizeObserver | null = null;

function statusLabel(s: string): string {
  return s === "running" ? t("cs.stRunning") : s === "failed" ? t("cs.stFailed") : t("cs.stDone");
}

function shortCmd(cmd: string): string {
  return cmd.length > 24 ? cmd.slice(0, 24) + "…" : cmd;
}

function isErrorLine(line: string): boolean {
  const l = line.toUpperCase();
  return l.includes("ERROR") || l.includes("BUILD FAILURE") || l.includes("FATAL");
}

/**
 * 问题列表：从运行输出中抽取诊断行（编译错误 / 警告）。
 * 兼容 Maven `[ERROR] /path/File.java:[12,5] msg` 与 Gradle `File.java:12: error: msg` 两种格式。
 */
interface Problem {
  msg: string;
  loc: string;
}

const problems = computed<Problem[]>(() => {
  const out: Problem[] = [];
  for (const run of consoleStore.runs) {
    for (const l of run.lines) {
      const text = l.line.trim();
      if (!text) continue;
      // Maven：...[ERROR] /src/a.java:[12,5] 语法错误
      const mvn = text.match(/\[(?:ERROR|WARNING)\]\s+(.+?):\[(\d+),(\d+)\]\s+(.*)$/);
      if (mvn) {
        out.push({ loc: `${mvn[1].split(/[\\/]/).pop()}:${mvn[2]}`, msg: `${mvn[3].trim()} ${mvn[4]}` });
        continue;
      }
      // Gradle：/src/a.java:12: error: msg
      const gradle = text.match(/^(.+?\.java):(\d+):?\s*(error|warning)?:?\s*(.*)$/i);
      if (gradle && /error|warning/i.test(text)) {
        out.push({ loc: `${gradle[1].split(/[\\/]/).pop()}:${gradle[2]}`, msg: gradle[4].trim() });
        continue;
      }
      // 兜底：含 ERROR/BUILD FAILURE 的行
      if (isErrorLine(text)) {
        out.push({ loc: "", msg: text.replace(/^\[?(ERROR|WARNING|FATAL)\]?[:\s]*/i, "").slice(0, 200) });
      }
    }
  }
  return out.slice(0, 200);
});

function startDrag(e: MouseEvent) {
  e.preventDefault();
  // 从收起态拖出时先展开，否则 height 计算基准不对
  ui.consoleVisible = true;
  const startY = e.clientY;
  const startH = height.value;
  // 拖拽期间禁掉全局过渡，保证 1:1 跟手（.cs-panel 本身已无 height 过渡，这里防的是子元素 hover）
  document.body.classList.add("is-resizing", "is-row-resizing");
  const onMove = (ev: MouseEvent) => {
    height.value = Math.min(480, Math.max(120, startH - (ev.clientY - startY)));
  };
  const onUp = () => {
    document.body.classList.remove("is-resizing", "is-row-resizing");
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ---- 输出虚拟滚动 ----
/** 懒加载样式的注入器：同一份 CSS 只注入一次 */
let xtermStyleInjected = false;
function injectStyle(css: string) {
  if (xtermStyleInjected) return;
  xtermStyleInjected = true;
  const el = document.createElement("style");
  el.dataset.emd = "xterm";
  el.textContent = css;
  document.head.appendChild(el);
}

/** 打平成等高行：每条 run 的「命令头」也是一行，从而整个列表可以统一虚拟化 */
interface OutputRow {
  key: string;
  text: string;
  err: boolean;
  head?: boolean;
  status?: RunStatus;
  code?: number | null;
}

const lineProbe = ref<HTMLElement>();

const outputRows = computed<OutputRow[]>(() => {
  const rows: OutputRow[] = [];
  for (const run of consoleStore.runs) {
    rows.push({
      key: `${run.id}:head`,
      text: run.command,
      err: false,
      head: true,
      status: run.status,
      code: run.code,
    });
    for (let i = 0; i < run.lines.length; i++) {
      const l = run.lines[i];
      rows.push({ key: `${run.id}:${i}`, text: l.line, err: l.stderr || isErrorLine(l.line) });
    }
  }
  return rows;
});

const rowCount = computed(() => outputRows.value.length);
const vl = useVirtualList(rowCount, lineProbe);

const visibleRows = computed(() => outputRows.value.slice(vl.start.value, vl.end.value));

/** 是否已贴底：贴底时新输出自动跟随，否则静默累计 */
const atBottom = ref(true);
watch(
  () => rowCount.value,
  () => {
    if (atBottom.value) void nextTick(() => jumpToBottom());
  },
);

function jumpToBottom() {
  const el = bodyEl.value;
  if (!el) return;
  atBottom.value = true;
  el.scrollTop = el.scrollHeight;
}

/** 滚到底部附近时恢复自动跟随，离开底部则显示「回到底部」按钮 */
function onOutputScroll() {
  vl.onScroll();
  const el = bodyEl.value;
  if (!el) return;
  atBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
}

const DEFAULT_H = 200;function resetHeight() {
  ui.consoleVisible = true;
  height.value = DEFAULT_H;
}

/** 键盘调节控制台高度：↑/↓ 每步 10px，Home 复位 */
function onResizerKey(e: KeyboardEvent) {
  const step = e.shiftKey ? 40 : 10;
  if (e.key === "ArrowUp") height.value = Math.min(480, height.value + step);
  else if (e.key === "ArrowDown") height.value = Math.max(120, height.value - step);
  else if (e.key === "Home") height.value = DEFAULT_H;
  else return;
  e.preventDefault();
  ui.consoleVisible = true;
}

// ---- 终端 ----
async function ensureTerminal() {
  if (term || !termHost.value) return;
  const [{ Terminal: T }, { FitAddon: F }] = await Promise.all([
    import("@xterm/xterm"),
    import("@xterm/addon-fit"),
    // xterm 的 CSS 也随终端懒加载：静态 import 会把约 8KB 样式塞进首屏 CSS
    import("@xterm/xterm/css/xterm.css?inline").then((m) => injectStyle(m.default)),
  ]);
  const host = termHost.value;
  const cs = getComputedStyle(document.documentElement);
  const v = (name: string, fallback: string) => cs.getPropertyValue(name).trim() || fallback;
  const t0 = new T({
    fontFamily: "JetBrains Mono, Cascadia Code, Consolas, monospace",
    fontSize: 13,
    cursorBlink: true,
    theme: {
      background: v("--background-primary", "#1e1e1e"),
      foreground: v("--text-normal", "#dcddde"),
      cursor: v("--interactive-accent", "#8b6cef"),
      selectionBackground: v("--selection-background", "#4d5b6e"),
    },
  });
  const f0 = new F();
  t0.loadAddon(f0);
  t0.open(host);
  t0.element?.focus();
  t0.onData((d) => {
    if (termId) void api.terminalWrite(termId, d);
  });
  term = t0;
  fit = f0;
  termDead.value = false;
  fit.fit();
  try {
    termId = await api.terminalCreate(t0.cols, t0.rows);
  } catch (e) {
    t0.writeln(`\x1b[31m终端创建失败：${String(e)}\x1b[0m`);
    return;
  }
  ro = new ResizeObserver(() => {
    if (!fit || !term) return;
    try {
      fit.fit();
      void api.terminalResize(termId, term.cols, term.rows);
    } catch {
      /* fit 未就绪时忽略 */
    }
  });
  ro.observe(host);
}

function killTerm() {
  if (termId) void api.terminalKill(termId);
}

function restartTerm() {
  disposeTerm();
  void nextTick(ensureTerminal);
}

function disposeTerm() {
  ro?.disconnect();
  ro = null;
  term?.dispose();
  term = null;
  fit = null;
  termId = 0;
  termDead.value = false;
}

// 输出/退出事件 → xterm（base64 → UTF-8 字节 → 文本）
function decodeB64(b64: string): string {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

onMounted(async () => {
  unOut = await api.onTerminalOut((e) => {
    if (e.id === termId && term) term.write(decodeB64(e.data));
  });
  unExit = await api.onTerminalExit((e) => {
    if (e.id === termId && term) {
      term.write(`\r\n\x1b[90m[会话已结束 exit ${e.code ?? "null"}]\x1b[0m\r\n`);
      termDead.value = true;
      termId = 0;
    }
  });
});

onUnmounted(() => {
  unOut?.();
  unExit?.();
  disposeTerm();
});

watch(
  () => [ui.consoleTab, ui.consoleVisible],
  ([tab, visible]) => {
    if (tab === "terminal" && visible) {
      void nextTick(() => ensureTerminal());
    }
  },
);
</script>

<style scoped>
.cs-problem {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 3px 8px;
  font-size: var(--font-ui-size);
  line-height: 1.5;
}
.cs-problem:hover {
  background: var(--background-modifier-hover);
}
.cs-problem-icon {
  color: var(--text-warning);
  flex-shrink: 0;
  margin-top: 2px;
}
.cs-problem-loc {
  flex-shrink: 0;
  color: var(--text-accent, #8b6cef);
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
}
.cs-problem-msg {
  color: var(--text-muted);
  word-break: break-word;
}
.cs-panel {
  position: relative;
  border-top: 1px solid var(--background-modifier-border);
  background: var(--background-primary);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow: hidden;
  /* 收起时只留页签条高度（33px），页面结构始终在位。
     不对 height 做过渡 —— 动 height 会触发 reflow，且拖拽调高时会全程追赶（红线2）。 */
}
/* 收起态：仅显示页签栏 */
.cs-panel.is-collapsed {
  height: 33px !important;
}
.cs-panel.is-collapsed .cs-body,
.cs-panel.is-collapsed .cs-term-body {
  display: none;
}
.cs-resizer {
  position: absolute;
  top: -3px;
  left: 0;
  right: 0;
  height: 6px;
  cursor: row-resize;
  z-index: 20;
}
.cs-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 10px;
  background: var(--background-secondary);
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.cs-tabs {
  display: flex;
  gap: var(--space-1);
}
/* 页签本体由 BaseTabs 提供（含徽标与按压反馈） */
.cs-running {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.cs-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.cs-kill {
  height: 22px;
  padding: 0 var(--space-3);
  font-size: var(--font-ui-smaller);
  color: var(--text-error);
  transition: background var(--anim-hover), transform var(--anim-press);
}
.cs-kill:hover {
  background: var(--background-modifier-hover);
}
.cs-kill:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .cs-kill:active {
    transform: none;
  }
}
.cs-tool-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.cs-tool-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.cs-tool-btn:active {
  transform: scale(0.92);
}
@media (hover: none) {
  .cs-tool-btn:active {
    transform: none;
  }
}
.cs-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0 8px;
}
.cs-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 18px 8px;
  font-size: var(--font-ui-size);
}
/* ---- 输出虚拟滚动 ----
   行是等高的（固定 line-height），所以虚拟化成立。
   命令头与输出行共用同一行高，才能放进同一个可视窗口。 */
.cs-virtual {
  position: relative;
  padding: 0;
}
.cs-vp {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 4px 0 8px;
}
.cs-vp-inner {
  position: relative;
  width: 100%;
}
.cs-vp-win {
  position: absolute;
  inset: 0 0 auto 0;
  will-change: transform;
}
.cs-line {
  display: block;
  height: 18px;
  line-height: 18px;
  padding: 0 10px;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  white-space: pre;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* 探针：离屏但保留布局，用于实测行高 */
.cs-probe {
  position: absolute;
  visibility: hidden;
  pointer-events: none;
  top: 0;
  left: 0;
}
/* 命令头行：与普通行同高，靠内层 flex 排布 */
.cs-line.is-head {
  font-family: var(--font-ui);
  background: var(--background-secondary);
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
.cs-jump-bottom {
  position: absolute;
  right: var(--space-4);
  bottom: var(--space-4);
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: var(--background-secondary);
  border: 1px solid var(--background-modifier-border);
  color: var(--text-muted);
  box-shadow: var(--shadow-l1);
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.cs-jump-bottom:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.cs-jump-bottom:active {
  transform: scale(0.92);
}
@media (hover: none) {
  .cs-jump-bottom:active {
    transform: none;
  }
}
.cs-status {
  flex-shrink: 0;
  font-size: var(--font-ui-smaller);
  border-radius: var(--radius-s);
  padding: 1px 6px;
  background: var(--background-modifier-hover);
  color: var(--text-muted);
}
.cs-status.is-running {
  background: var(--interactive-accent-hover-alt);
  color: var(--interactive-accent);
}
.cs-status.is-failed {
  background: rgba(233, 49, 71, 0.14);
  color: var(--text-error);
}
.cs-cmd {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  color: var(--text-normal);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cs-code {
  margin-left: auto;
  flex-shrink: 0;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  font-family: var(--font-mono);
}
.cs-line.is-err {
  color: var(--text-error);
}
.cs-term-body {
  flex: 1;
  min-height: 0;
  padding: 2px 6px;
}
.cs-term-host {
  height: 100%;
}
</style>
