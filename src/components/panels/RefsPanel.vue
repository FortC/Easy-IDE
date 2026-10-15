<template>
  <div class="refs-panel">
    <div class="emd-panel-header">
      <span>{{ t("rf.title") }}</span>
      <span v-if="anchors.length > 0" class="rf-n">{{ refs.length }}</span>
    </div>

    <!-- 未打开代码文件：提示 -->
    <div v-if="!isCodeFile" class="rf-empty">{{ t("rf.noFile") }}</div>

    <template v-else>
      <!-- 定位锚点：当前文件的类/接口名（取最外层符号） -->
      <div v-if="anchors.length > 0" class="rf-anchors">
        <button
          v-for="a in anchors"
          :key="a"
          class="rf-anchor"
          :class="{ 'is-on': anchor === a }"
          :title="t('rf.anchorTitle')"
          @click="anchor = a"
        >
          {{ a }}
        </button>
      </div>
      <div v-else class="rf-empty">{{ t("rf.noSymbol") }}</div>

      <!-- 查找结果：跨文件符号命中 -->
      <div v-if="anchor && searching" class="rf-empty">{{ t("rf.searching") }}</div>
      <div v-else-if="refs.length === 0 && anchor" class="rf-empty">{{ t("rf.empty") }}</div>
      <div v-else class="rf-scroll">
        <div v-for="r in refs" :key="r.path + ':' + r.name + ':' + r.line" class="emd-tree-item rf-item" @click="jump(r)">
          <span class="rf-kind" :class="'k-' + r.kind">{{ kindLabel(r.kind) }}</span>
          <span class="rf-text">{{ r.name }}</span>
          <span class="rf-path">{{ shortPath(r.path) }}</span>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useEditorStore } from "../../stores/editor";
import { api } from "../../ipc/tauri";
import { isMarkdown } from "../../lib/filetypes";
import type { CodeSymbol, SymbolHit } from "../../types";
import { t } from "../../i18n";

const editor = useEditorStore();

const isCodeFile = computed(() => !!editor.activePath && !isMarkdown(editor.activePath));
const fileSymbols = ref<CodeSymbol[]>([]);
/** 定位锚点：当前文件最外层符号（class/interface/enum/record），没有则取首个方法 */
const anchors = computed(() => {
  const outer = fileSymbols.value.filter((s) => OUTER_KINDS.includes(s.kind) && s.name);
  if (outer.length > 0) return Array.from(new Set(outer.map((s) => s.name))).slice(0, 5);
  const any = fileSymbols.value.filter((s) => s.name && !s.container);
  return Array.from(new Set(any.map((s) => s.name))).slice(0, 5);
});

const OUTER_KINDS = ["class", "interface", "enum", "record"];

const anchor = ref("");
const refs = ref<SymbolHit[]>([]);
const searching = ref(false);
let seq = 0;

// 当前文件符号变化 → 更新锚点候选
watch(
  () => [editor.activePath, editor.savedContent] as const,
  async () => {
    const path = editor.activePath;
    if (!path || isMarkdown(path)) {
      fileSymbols.value = [];
      refs.value = [];
      return;
    }
    fileSymbols.value = await api.codeFileSymbols(path);
  },
  { immediate: true },
);

// 锚点变化 → 跨文件符号检索
watch([anchor, () => editor.activePath], async () => {
  const path = editor.activePath;
  if (!anchor.value || !path) {
    refs.value = [];
    return;
  }
  const token = ++seq;
  searching.value = true;
  try {
    const hits = await api.codeSymbolSearch(anchor.value, null, 50);
    // 当前文件自身已在左栏树可见，过滤掉；同名其他定义为「实现位置」
    refs.value = hits.filter((h) => h.path !== path);
  } catch {
    refs.value = [];
  } finally {
    if (token === seq) searching.value = false;
  }
});

function shortPath(p: string): string {
  const parts = p.split("/");
  return parts.length <= 2 ? p : ".../" + parts.slice(-2).join("/");
}

function jump(r: SymbolHit) {
  void editor.openNote(r.path);
  void editor.save();
  // 保存后由 pendingJump 定位行号
  (editor as unknown as { pendingJump: { line?: number } | null }).pendingJump = {
    line: Math.max(0, r.line - 1),
  };
}

function kindLabel(kind: string): string {
  const map: Record<string, string> = {
    class: "C",
    interface: "I",
    enum: "E",
    record: "R",
    method: "M",
    function: "F",
    field: "A",
  };
  return map[kind] ?? "?";
}
</script>

<style scoped>
.refs-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.rf-anchors {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.rf-anchor {
  padding: 2px 8px;
  border-radius: var(--radius-s);
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  font-family: var(--font-mono);
  transition: background var(--anim-hover), color var(--anim-hover),
    border-color var(--anim-hover);
}
.rf-anchor:hover {
  color: var(--text-normal);
}
.rf-anchor.is-on {
  background: var(--interactive-accent);
  border-color: var(--interactive-accent);
  color: var(--text-on-accent);
}
.rf-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 0;
}
.rf-n {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.rf-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  height: var(--list-item-height);
  cursor: pointer;
}
.rf-kind {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-faint);
  width: 12px;
  flex-shrink: 0;
}
.rf-kind.k-class,
.rf-kind.k-interface {
  color: var(--interactive-accent);
}
.rf-text {
  flex-shrink: 0;
  max-width: 55%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-ui-size);
}
.rf-path {
  flex: 1;
  min-width: 0;
  text-align: right;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rf-empty {
  padding: 16px 12px;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  text-align: center;
}
</style>
