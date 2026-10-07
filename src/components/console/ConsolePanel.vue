<template>
  <div v-if="ui.consoleVisible" class="cs-panel" :style="{ height: height + 'px' }">
    <div class="cs-resizer" @mousedown="startDrag" />
    <div class="cs-head">
      <span class="cs-title">
        <Icon name="terminal" :size="13" />
        {{ t("cs.title") }}
      </span>
      <span v-if="consoleStore.running.length > 0" class="cs-running">
        {{ tf("cs.running", { n: consoleStore.running.length }) }}
      </span>
      <div class="cs-actions">
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
        <button class="cs-tool-btn" :title="t('cs.hide')" @click="ui.consoleVisible = false">
          <Icon name="x" :size="13" />
        </button>
      </div>
    </div>
    <div ref="bodyEl" class="cs-body">
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
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import { useUiStore } from "../../stores/ui";
import { useConsoleStore } from "../../stores/console";
import { t, tf } from "../../i18n";

const ui = useUiStore();
const consoleStore = useConsoleStore();
const height = ref(200);
const bodyEl = ref<HTMLElement>();

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

// 新输出 → 滚到底
watch(
  () => consoleStore.runs.map((r) => r.lines.length).join(","),
  () => {
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
  padding: 4px 10px;
  background: var(--background-secondary);
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.cs-title {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-normal);
  font-weight: 600;
  font-size: var(--font-ui-size);
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
</style>
