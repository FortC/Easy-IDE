<template>
  <div v-if="ui.consoleVisible" class="cs-panel" :style="{ height: height + 'px' }">
    <div class="cs-resizer" @mousedown="startDrag" />
    <div class="cs-head">
      <div class="cs-tabs">
        <button
          class="cs-tab"
          :class="{ 'is-active': ui.consoleTab === 'output' }"
          @click="ui.consoleTab = 'output'"
        >
          {{ t("cs.tabOutput") }}
        </button>
        <button
          class="cs-tab"
          :class="{ 'is-active': ui.consoleTab === 'terminal' }"
          @click="ui.consoleTab = 'terminal'"
        >
          {{ t("cs.tabTerminal") }}
        </button>
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
        <template v-else>
          <button v-if="termDead" class="emd-btn" :title="t('cs.termRestart')" @click="restartTerm">
            <Icon name="refresh-cw" :size="11" /> {{ t("cs.termRestart") }}
          </button>
          <button class="cs-tool-btn" :title="t('cs.kill')" @click="killTerm">
            <Icon name="square" :size="13" />
          </button>
        </template>
        <button class="cs-tool-btn" :title="t('cs.hide')" @click="ui.consoleVisible = false">
          <Icon name="x" :size="13" />
        </button>
      </div>
    </div>

    <!-- 输出视图 -->
    <div v-show="ui.consoleTab === 'output'" ref="bodyEl" class="cs-body">
      <div v-if="consoleStore.runs.length === 0" class="cs-empty">{{ t("cs.empty") }}</div>
      <div v-for="run in consoleStore.runs" :key="run.id" class="cs-run">
        <div class="cs-run-head">
          <span class="cs-status" :class="'is-' + run.status">{{ statusLabel(run.status) }}</span>
          <span class="cs-cmd">{{ run.command }}</span>
          <span v-if="run.status !== 'running' && run.code !== null" class="cs-code">
            exit {{ run.code }}
          </span>
        </div>
        <pre class="cs-lines"><span
          v-for="(l, i) in run.lines"
          :key="i"
          class="cs-line"
          :class="{ 'is-err': l.stderr || isErrorLine(l.line) }"
        >{{ l.line }}</span></pre>
      </div>
    </div>

    <!-- 终端视图 -->
    <div v-show="ui.consoleTab === 'terminal'" class="cs-term-body">
      <div ref="termHost" class="cs-term-host" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { Terminal } from "@xterm/xterm";
import type { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import Icon from "../common/Icon.vue";
import { useUiStore } from "../../stores/ui";
import { useConsoleStore } from "../../stores/console";
import { api } from "../../ipc/tauri";
import { t, tf } from "../../i18n";

const ui = useUiStore();
const consoleStore = useConsoleStore();
const height = ref(200);
const bodyEl = ref<HTMLElement>();
const termHost = ref<HTMLElement>();
const termDead = ref(false);

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

function startDrag(e: MouseEvent) {
  e.preventDefault();
  const startY = e.clientY;
  const startH = height.value;
  const onMove = (ev: MouseEvent) => {
    height.value = Math.min(480, Math.max(120, startH - (ev.clientY - startY)));
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ---- 终端 ----
async function ensureTerminal() {
  if (term || !termHost.value) return;
  const [{ Terminal: T }, { FitAddon: F }] = await Promise.all([
    import("@xterm/xterm"),
    import("@xterm/addon-fit"),
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

// 输出流滚到底
watch(
  () => consoleStore.runs.map((r) => r.lines.length).join(","),
  () => {
    if (ui.consoleTab !== "output") return;
    void nextTick(() => {
      bodyEl.value?.scrollTo({ top: bodyEl.value.scrollHeight });
    });
  },
);
</script>

<style scoped>
.cs-panel {
  position: relative;
  border-top: 1px solid var(--background-modifier-border);
  background: var(--background-primary);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow: hidden;
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
  gap: 2px;
}
.cs-tab {
  height: 22px;
  padding: 0 10px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
}
.cs-tab:hover {
  color: var(--text-normal);
}
.cs-tab.is-active {
  background: var(--background-modifier-active-hover);
  color: var(--text-normal);
}
.cs-running {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.cs-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 6px;
}
.cs-kill {
  height: 22px;
  padding: 0 8px;
  font-size: var(--font-ui-smaller);
  color: var(--text-error, #e93147);
}
.cs-tool-btn {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.cs-tool-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
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
.cs-run {
  border-bottom: 1px solid var(--background-modifier-border);
  padding: 4px 0;
}
.cs-run:last-child {
  border-bottom: none;
}
.cs-run-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 10px;
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
  color: var(--text-error, #e93147);
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
.cs-lines {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  line-height: 1.5;
  padding: 2px 10px;
  overflow-x: auto;
  white-space: pre;
}
.cs-line {
  display: block;
  color: var(--text-muted);
}
.cs-line.is-err {
  color: var(--text-error, #e93147);
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
