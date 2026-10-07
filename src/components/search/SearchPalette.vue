<template>
  <transition name="emd-fade">
    <div v-if="ui.searchOpen" class="emd-modal-bg" @click.self="close">
    <div class="emd-modal sp-modal">
      <div class="sp-tabs">
        <button
          v-for="m in modeTabs"
          :key="m.key"
          class="sp-tab"
          :class="{ 'is-active': mode === m.key }"
          @click="switchMode(m.key)"
        >
          {{ m.label }}
        </button>
      </div>
      <input
        ref="inputRef"
        v-model="query"
        type="text"
        class="sp-input"
        :placeholder="placeholder"
        @keydown.down.prevent="move(1)"
        @keydown.up.prevent="move(-1)"
        @keydown.enter="chooseSelected"
        @keydown.esc="close"
      />
      <div class="sp-results">
        <!-- 文件（Ctrl+P） -->
        <template v-if="mode === 'file'">
          <div
            v-for="(h, i) in fileHits"
            :key="h.path"
            class="sp-item"
            :class="{ 'is-selected': i === selected }"
            @mousemove="selected = i"
            @click="openAt(h.path)"
          >
            <Icon :name="iconForFile(h.path)" :size="14" />
            <span class="sp-title">{{ h.name }}</span>
            <span class="sp-sub">{{ h.path }}</span>
          </div>
        </template>

        <!-- 类 / 符号（Ctrl+N / Ctrl+Shift+Alt+N） -->
        <template v-else-if="mode === 'class' || mode === 'symbol'">
          <div
            v-for="(h, i) in symbolHits"
            :key="h.path + h.name + h.line"
            class="sp-item"
            :class="{ 'is-selected': i === selected }"
            @mousemove="selected = i"
            @click="openAt(h.path, h.line)"
          >
            <span class="sp-kind" :class="'k-' + h.kind">{{ kindLabel(h.kind) }}</span>
            <span class="sp-title">{{ h.name }}</span>
            <span class="sp-sub">
              {{ h.container ? h.container + " · " : "" }}{{ h.path }}:{{ h.line }}
            </span>
          </div>
        </template>

        <!-- 全文（Ctrl+Shift+F） -->
        <template v-else>
          <div
            v-for="(h, i) in textHits"
            :key="h.path + h.line_no"
            class="sp-item sp-item-content"
            :class="{ 'is-selected': i === selected }"
            @mousemove="selected = i"
            @click="openAt(h.path, h.line_no)"
          >
            <div class="sp-line1">
              <Icon :name="iconForFile(h.path)" :size="14" />
              <span class="sp-title">{{ fileNameOf(h.path) }}</span>
              <span class="sp-ln">:{{ h.line_no }}</span>
            </div>
            <div class="sp-text">{{ h.text }}</div>
          </div>
        </template>

        <div v-if="searched && hitCount === 0" class="sp-empty">
          {{ t("sp.none") }}
        </div>
        <div v-if="!searched" class="sp-empty">{{ hint }}</div>
      </div>
    </div>
  </div>
  </transition>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import { useUiStore } from "../../stores/ui";
import { useEditorStore } from "../../stores/editor";
import { iconForFile } from "../../lib/filetypes";
import { t } from "../../i18n";
import { api } from "../../ipc/tauri";
import type { CodeFileHit, SymbolHit, TextHit } from "../../types";

type PaletteMode = "file" | "class" | "symbol" | "content";

const ui = useUiStore();
const editor = useEditorStore();
const query = ref("");
const mode = ref<PaletteMode>("file");
const selected = ref(0);
const searched = ref(false);
const inputRef = ref<HTMLInputElement>();

const fileHits = ref<CodeFileHit[]>([]);
const symbolHits = ref<SymbolHit[]>([]);
const textHits = ref<TextHit[]>([]);

let timer: ReturnType<typeof setTimeout> | null = null;
let seq = 0; // 防乱序：慢请求返回时丢弃

const modeTabs = computed(() => [
  { key: "file" as const, label: t("sp.file") },
  { key: "class" as const, label: t("sp.class") },
  { key: "symbol" as const, label: t("sp.symbol") },
  { key: "content" as const, label: t("sp.content") },
]);

const hitCount = computed(() =>
  mode.value === "file"
    ? fileHits.value.length
    : mode.value === "content"
      ? textHits.value.length
      : symbolHits.value.length,
);

const placeholder = computed(() =>
  mode.value === "file"
    ? t("sp.filePh")
    : mode.value === "class"
      ? t("sp.classPh")
      : mode.value === "symbol"
        ? t("sp.symbolPh")
        : t("sp.contentPh"),
);

const hint = computed(() =>
  mode.value === "file"
    ? t("sp.fileHint")
    : mode.value === "class"
      ? t("sp.classHint")
      : mode.value === "symbol"
        ? t("sp.symbolHint")
        : t("sp.contentHint"),
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
  return map[kind] ?? kind.slice(0, 1).toUpperCase();
}

function fileNameOf(path: string): string {
  return path.split("/").pop() || path;
}

watch(query, (q) => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => doSearch(q), 150);
});

watch(mode, () => {
  query.value = "";
  searched.value = false;
  fileHits.value = [];
  symbolHits.value = [];
  textHits.value = [];
});

async function doSearch(q: string) {
  selected.value = 0;
  const token = ++seq;
  if (!q.trim()) {
    searched.value = false;
    fileHits.value = [];
    symbolHits.value = [];
    textHits.value = [];
    return;
  }
  if (mode.value === "file") {
    const hits = await api.codeFileSearch(q, 50);
    if (token === seq) fileHits.value = hits;
  } else if (mode.value === "class") {
    const hits = await api.codeSymbolSearch(q, ["class", "interface", "enum", "record"], 50);
    if (token === seq) symbolHits.value = hits;
  } else if (mode.value === "symbol") {
    const hits = await api.codeSymbolSearch(q, null, 50);
    if (token === seq) symbolHits.value = hits;
  } else {
    const hits = await api.codeTextSearch(q, false, 300);
    if (token === seq) textHits.value = hits;
  }
  if (token === seq) searched.value = true;
}

function switchMode(m: PaletteMode) {
  mode.value = m;
  nextTick(() => inputRef.value?.focus());
}

function move(delta: number) {
  const n = hitCount.value;
  if (!n) return;
  selected.value = (selected.value + delta + n) % n;
}

async function chooseSelected() {
  if (mode.value === "file") {
    const h = fileHits.value[selected.value];
    if (h) await openAt(h.path);
  } else if (mode.value === "content") {
    const h = textHits.value[selected.value];
    if (h) await openAt(h.path, h.line_no);
  } else {
    const h = symbolHits.value[selected.value];
    if (h) await openAt(h.path, h.line);
  }
}

/** 打开文件并定位（line 为 1 基；editor 跳转用 0 基） */
async function openAt(path: string, line?: number) {
  close();
  await editor.openNote(path, line !== undefined ? { line: line - 1 } : null);
}

function close() {
  ui.searchOpen = false;
}

onMounted(() => {
  mode.value = ui.searchMode;
  nextTick(() => inputRef.value?.focus());
});
</script>

<style scoped>
.sp-modal {
  width: 620px;
  margin-top: -15vh;
}
.sp-tabs {
  display: flex;
  gap: 4px;
  padding: 10px 12px 0;
}
.sp-tab {
  padding: 5px 12px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.sp-tab:hover {
  background: var(--background-modifier-hover);
}
.sp-tab.is-active {
  background: var(--background-modifier-active-hover);
  color: var(--text-normal);
}
.sp-input {
  margin: 10px 12px;
  width: calc(100% - 24px);
}
.sp-results {
  flex: 1;
  overflow-y: auto;
  padding: 4px 8px 10px;
  min-height: 120px;
  max-height: 50vh;
}
.sp-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: var(--radius-s);
  cursor: pointer;
}
.sp-item.is-selected {
  background: var(--background-modifier-active-hover);
}
.sp-item-content {
  flex-direction: column;
  align-items: stretch;
  gap: 3px;
}
.sp-line1 {
  display: flex;
  align-items: center;
  gap: 8px;
}
.sp-title {
  color: var(--text-normal);
  white-space: nowrap;
}
.sp-sub,
.sp-ln {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sp-sub {
  flex: 1;
}
.sp-text {
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  padding-left: 22px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
}
.sp-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 24px 8px;
}
/* 符号类型徽标（IDEA 风格字母角标） */
.sp-kind {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  font-size: 11px;
  font-weight: 700;
  background: var(--background-modifier-hover);
  color: var(--text-muted);
}
.sp-kind.k-class,
.sp-kind.k-interface,
.sp-kind.k-enum,
.sp-kind.k-record {
  background: var(--interactive-accent-hover-alt);
  color: var(--interactive-accent);
}
.sp-kind.k-method,
.sp-kind.k-function {
  background: var(--tag-background);
  color: var(--tag-color);
}
</style>
