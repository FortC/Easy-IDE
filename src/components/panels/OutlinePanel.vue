<template>
  <div class="outline-panel">
    <div class="emd-panel-header"><span>{{ t("ol.title") }}</span></div>
    <div class="ol-list">
      <!-- markdown：标题大纲 -->
      <template v-if="isMd">
        <div
          v-for="(h, i) in headings"
          :key="i"
          class="emd-tree-item"
          :style="{ paddingLeft: 6 + (h.level - 1) * 12 + 'px' }"
          @click="gotoLine(h.line)"
        >
          <span class="ol-text">{{ h.text || t("ol.emptyHeading") }}</span>
        </div>
        <div v-if="headings.length === 0" class="ol-empty">{{ t("ol.empty") }}</div>
      </template>

      <!-- 代码：符号大纲 -->
      <template v-else>
        <div
          v-for="(s, i) in symbols"
          :key="i"
          class="emd-tree-item ol-sym"
          :style="{ paddingLeft: 6 + (s.container ? 1 : 0) * 14 + 'px' }"
          @click="gotoLine(s.line - 1)"
        >
          <span class="ol-kind" :class="'k-' + s.kind">{{ kindLabel(s.kind) }}</span>
          <span class="ol-text">{{ s.name }}</span>
        </div>
        <div v-if="symbols.length === 0" class="ol-empty">{{ t("ol.emptyCode") }}</div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useEditorStore } from "../../stores/editor";
import { parseHeadings } from "../../lib/markdown/renderer";
import { isMarkdown } from "../../lib/filetypes";
import { api } from "../../ipc/tauri";
import type { CodeSymbol } from "../../types";
import { t } from "../../i18n";

const editor = useEditorStore();

const isMd = computed(() => isMarkdown(editor.activePath));
const headings = computed(() =>
  editor.activePath ? parseHeadings(editor.content) : [],
);

const symbols = ref<CodeSymbol[]>([]);
let symTimer: ReturnType<typeof setTimeout> | null = null;
let symSeq = 0;

// 代码文件：切换文件或内容保存（磁盘索引更新）后刷新符号
watch(
  () => [editor.activePath, editor.savedContent] as const,
  () => {
    if (symTimer) clearTimeout(symTimer);
    symTimer = setTimeout(async () => {
      const path = editor.activePath;
      if (!path || isMarkdown(path)) {
        symbols.value = [];
        return;
      }
      const token = ++symSeq;
      const list = await api.codeFileSymbols(path);
      if (token === symSeq) symbols.value = list;
    }, 250);
  },
  { immediate: true },
);

function kindLabel(kind: string): string {
  const map: Record<string, string> = {
    class: "C",
    interface: "I",
    enum: "E",
    record: "R",
    method: "M",
    function: "F",
    field: "P",
  };
  return map[kind] ?? "?";
}

function gotoLine(line: number) {
  if (editor.mode === "preview") {
    // 阅读模式无源码行可跳，切换到分屏后跳
    editor.setMode("split");
    setTimeout(() => window.dispatchEvent(new CustomEvent("emd-goto-line", { detail: line })), 80);
  } else {
    window.dispatchEvent(new CustomEvent("emd-goto-line", { detail: line }));
  }
}
</script>

<style scoped>
.outline-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.ol-list {
  flex: 1;
  overflow-y: auto;
  padding: 2px 6px;
}
.ol-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-ui-size);
}
.ol-sym {
  gap: 6px;
}
.ol-kind {
  flex-shrink: 0;
  width: 17px;
  height: 17px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  font-size: 10px;
  font-weight: 700;
  background: var(--background-modifier-hover);
  color: var(--text-muted);
}
.ol-kind.k-class,
.ol-kind.k-interface,
.ol-kind.k-enum,
.ol-kind.k-record {
  background: var(--interactive-accent-hover-alt);
  color: var(--interactive-accent);
}
.ol-kind.k-method,
.ol-kind.k-function {
  background: var(--tag-background);
  color: var(--tag-color);
}
.ol-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 16px 8px;
}
</style>
