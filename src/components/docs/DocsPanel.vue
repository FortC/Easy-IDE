<template>
  <div class="docs-panel">
    <div class="emd-panel-header">
      <span>{{ t("rb.docs") }}</span>
      <span class="dc-count">{{ docs.visibleEntries.length }}</span>
    </div>

    <!-- 工具栏 -->
    <div class="dc-toolbar">
      <button class="dc-tool-btn" :title="t('dc.refresh')" :disabled="docs.loading" @click="docs.scan()">
        <Icon name="refresh-cw" :size="13" :class="{ 'is-spin': docs.loading }" />
      </button>
      <select v-model="groupMode" class="dc-group-select" :title="t('dc.groupLabel')">
        <option value="dir">{{ t("dc.g.dir") }}</option>
        <option value="smart">{{ t("dc.g.smart") }}</option>
        <option value="flat">{{ t("dc.g.flat") }}</option>
        <option value="date">{{ t("dc.g.date") }}</option>
      </select>
      <button class="dc-tool-btn" :title="t('dc.aiClassify')" :disabled="!!docs.classify" @click="runAiClassify">
        <Icon name="play" :size="13" />
      </button>
      <button class="dc-tool-btn" :title="t('dc.agents')" @click="runAgents">
        <Icon name="file-code" :size="13" />
      </button>
      <button
        class="dc-tool-btn"
        :title="t('dc.settings')"
        :class="{ 'is-active': settingsOpen }"
        @click="settingsOpen = !settingsOpen"
      >
        <Icon name="settings" :size="13" />
      </button>
    </div>
    <div v-if="docs.classify" class="dc-progress">
      {{ tf("dc.classifying", { d: docs.classify.done, t: docs.classify.total }) }}
    </div>
    <div v-if="docs.notice" class="dc-notice" @click="docs.notice = ''">
      <Icon name="check" :size="12" />
      <span class="dc-notice-text">{{ docs.notice }}</span>
    </div>

    <!-- 设置区（折叠） -->
    <div v-if="settingsOpen" class="dc-settings">
      <div class="dc-set-title">{{ t("dc.knowledge") }}</div>
      <div class="dc-set-grid">
        <div v-for="(kd, i) in knowledgeEdit" :key="kd[0]" class="dc-set-row">
          <label class="dc-set-label">{{ roleLabel(kd[0]) }}</label>
          <input v-model="knowledgeEdit[i][1]" type="text" @change="saveKnowledge" />
        </div>
      </div>
      <label class="dc-set-check">
        <input v-model="aiDocsToggle" type="checkbox" @change="docs.toggleAiDocs(aiDocsToggle)" />
        {{ t("dc.showAi") }}
      </label>
      <div class="dc-set-title">{{ t("dc.aiDirs") }}</div>
      <textarea v-model="aiDirsEdit" class="dc-set-area" rows="4" @change="saveAiDirs" />
      <button v-if="docs.ignoredCount > 0" class="dc-unignore" @click="docs.unignoreAll()">
        {{ tf("dc.unignoreAll", { n: docs.ignoredCount }) }}
      </button>
    </div>

    <!-- 文档树 -->
    <div class="dc-tree">
      <template v-if="docs.groupMode !== 'flat'">
        <div v-for="g in groups" :key="g.key" class="dc-group">
          <div
            class="emd-tree-item dc-group-row"
            @click="toggleGroup(g.key)"
            @contextmenu.prevent="showMenu($event, null, g)"
          >
            <Icon :name="expanded.has(g.key) ? 'chevron-down' : 'chevron-right'" :size="12" />
            <span class="dc-group-name">{{ g.label }}</span>
            <span class="dc-group-count">{{ g.entries.length }}</span>
          </div>
          <template v-if="expanded.has(g.key)">
            <div
              v-for="e in g.entries"
              :key="e.path"
              class="emd-tree-item dc-file-row"
              :class="{ 'is-active': e.path === editor.activePath }"
              @click="openDoc(e)"
              @contextmenu.prevent="showMenu($event, e, null)"
            >
              <Icon :name="e.from_ai_dir ? 'settings-2' : 'file-text'" :size="13" class="dc-fileicon" />
              <span class="dc-file-name">{{ e.name }}</span>
              <span v-if="docs.groupMode === 'smart'" class="dc-file-dir">
                {{ parentDir(e.path) }}
              </span>
            </div>
          </template>
        </div>
      </template>
      <template v-else>
        <div
          v-for="e in docs.visibleEntries"
          :key="e.path"
          class="emd-tree-item dc-file-row"
          :class="{ 'is-active': e.path === editor.activePath }"
          @click="openDoc(e)"
          @contextmenu.prevent="showMenu($event, e, null)"
        >
          <Icon :name="e.from_ai_dir ? 'settings-2' : 'file-text'" :size="13" class="dc-fileicon" />
          <span class="dc-file-name">{{ e.name }}</span>
          <span class="dc-file-dir">{{ parentDir(e.path) }}</span>
        </div>
      </template>
      <div v-if="!docs.loading && docs.visibleEntries.length === 0" class="dc-empty">
        {{ t("dc.empty") }}
      </div>
      <div v-if="docs.loading" class="dc-empty">{{ t("dc.scanning") }}</div>
    </div>

    <!-- 右键菜单 -->
    <DropdownMenu
      :open="menu.visible"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      @select="onMenuSelect"
      @close="menu.visible = false"
    />

    <!-- 类别 / AI 操作 选择器 -->
    <transition name="emd-fade">
      <div v-if="picker.visible" class="emd-modal-bg" @click.self="picker.visible = false">
        <div class="emd-modal dc-picker">
          <div class="dc-picker-title">{{ picker.title }}</div>
          <input v-model="picker.query" type="text" class="dc-picker-input" :placeholder="t('dc.opFilter')" />
          <div class="dc-picker-list">
            <button
              v-for="o in pickerOptions"
              :key="o.key"
              class="dc-picker-item"
              @click="picker.visible = false, picker.onPick(o.key)"
            >
              {{ o.label }}
            </button>
          </div>
          <button class="dc-picker-cancel" @click="picker.visible = false">{{ t("c.cancel") }}</button>
        </div>
      </div>
    </transition>

    <!-- AI 操作结果 -->
    <transition name="emd-fade">
      <div v-if="result.visible" class="emd-modal-bg" @click.self="result.visible = false">
        <div class="emd-modal dc-result">
          <div class="dc-result-title">{{ t("dc.aiResult") }} · {{ result.opLabel }} · {{ result.docName }}</div>
          <textarea v-model="result.text" class="dc-result-area" readonly />
          <div class="dc-result-actions">
            <button class="emd-btn" @click="copyResult">{{ t("dc.copy") }}</button>
            <button class="emd-btn" @click="appendResult">{{ t("dc.append") }}</button>
            <button class="emd-btn emd-btn-accent" @click="saveAsNew">{{ t("dc.saveAs") }}</button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import DropdownMenu, { type DropItem } from "../common/DropdownMenu.vue";
import { useDocsStore, type DocsGroupMode } from "../../stores/docs";
import { useEditorStore } from "../../stores/editor";
import { useUiStore } from "../../stores/ui";
import { useAiStore } from "../../stores/ai";
import { api } from "../../ipc/tauri";
import type { DocEntry } from "../../types";
import { t, tf } from "../../i18n";

const docs = useDocsStore();
const editor = useEditorStore();
const ui = useUiStore();
const aiStore = useAiStore();

const settingsOpen = ref(false);
const knowledgeEdit = ref<[string, string][]>([]);
const aiDirsEdit = ref("");
const aiDocsToggle = ref(false);
const expanded = ref(new Set<string>());

const AI_OPS = computed(() => [
  { key: "summarize", label: t("dc.op.summarize") },
  { key: "translate", label: t("dc.op.translate") },
  { key: "polish", label: t("dc.op.polish") },
  { key: "outline", label: t("dc.op.outline") },
  { key: "todos", label: t("dc.op.todos") },
]);

function roleLabel(role: string): string {
  const map: Record<string, string> = { req: "需求", exa: "示例", res: "成果", err: "错误" };
  return map[role] ?? role;
}

const groupMode = computed({
  get: () => docs.groupMode,
  set: (v) => void docs.setGroupMode(v as DocsGroupMode),
});

interface Group {
  key: string;
  label: string;
  order: number;
  entries: DocEntry[];
}

/** 分组：按目录 / 智能分类 / 按日期（flat 在模板单独处理） */
const groups = computed<Group[]>(() => {
  const entries = docs.visibleEntries;
  if (docs.groupMode === "date") {
    const map = new Map<string, DocEntry[]>();
    for (const e of entries) {
      const k = e.date_key || "unknown";
      (map.get(k) ?? map.set(k, []).get(k)!).push(e);
    }
    return [...map.entries()]
      .sort((a, b) => b[0].localeCompare(a[0]))
      .map(([k, list]) => ({
        key: k,
        label: k,
        order: 0,
        entries: [...list].sort((a, b) => b.mtime - a.mtime),
      }));
  }
  if (docs.groupMode === "dir") {
    const map = new Map<string, DocEntry[]>();
    for (const e of entries) {
      const seg = e.path.split("/");
      const k = seg.length > 1 ? seg[0] : "";
      (map.get(k) ?? map.set(k, []).get(k)!).push(e);
    }
    return [...map.entries()]
      .sort((a, b) => a[0].toLowerCase().localeCompare(b[0].toLowerCase()))
      .map(([k, list]) => ({
        key: k,
        label: k || t("dc.rootDir"),
        order: 0,
        entries: list,
      }));
  }
  // smart：AI 工具文档 → 知识库目录（需求/示例/成果/错误）→ 其余类别 → 未分类
  const roleOrder = new Map(docs.knowledgeDirs.map((kd, i) => [kd[1], i]));
  const map = new Map<string, DocEntry[]>();
  const unclassifiedLabel = "未分类";
  for (const e of entries) {
    const k = e.category ?? unclassifiedLabel;
    (map.get(k) ?? map.set(k, []).get(k)!).push(e);
  }
  const groupsOut: Group[] = [];
  const orderOf = (k: string): number => {
    if (k === "AI 工具文档") return 0;
    if (roleOrder.has(k)) return 1;
    if (k === unclassifiedLabel) return 3;
    return 2;
  };
  for (const [k, list] of map) {
    groupsOut.push({
      key: k,
      label: k,
      order: orderOf(k),
      entries: [...list].sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase())),
    });
  }
  groupsOut.sort(
    (a, b) => a.order - b.order || (roleOrder.get(a.label) ?? 9) - (roleOrder.get(b.label) ?? 9) || a.label.localeCompare(b.label, "zh"),
  );
  return groupsOut;
});

function toggleGroup(key: string) {
  const s = new Set(expanded.value);
  if (s.has(key)) s.delete(key);
  else s.add(key);
  expanded.value = s;
}

function parentDir(path: string): string {
  const seg = path.split("/");
  return seg.length > 1 ? seg[seg.length - 2] : "";
}

async function openDoc(e: DocEntry) {
  await editor.openNote(e.path);
  ui.view = "editor";
}

// ---- 右键菜单 ----
const menu = reactive({
  visible: false,
  x: 0,
  y: 0,
  entry: null as DocEntry | null,
  group: null as Group | null,
});

function showMenu(ev: MouseEvent, entry: DocEntry | null, group: Group | null) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.entry = entry;
  menu.group = group;
  menu.visible = true;
}

const menuItems = computed<DropItem[]>(() => {
  if (menu.group) {
    return [
      { key: "zipGroup", label: t("dc.zipGroup"), icon: "package" },
    ];
  }
  const items: DropItem[] = [
    { key: "open", label: t("dc.open"), icon: "file-text" },
    { key: "archive", label: t("dc.archive"), icon: "folder" },
    { key: "aiOp", label: t("dc.aiOp"), icon: "play" },
    { key: "chat", label: t("dc.chat"), icon: "link" },
    { key: "sep1", label: "", separator: true },
    { key: "copyPath", label: t("dc.copyPath") },
    { key: "zip", label: t("dc.zip"), icon: "package" },
    { key: "sep2", label: "", separator: true },
    { key: "ignore", label: t("dc.ignore"), icon: "x" },
  ];
  return items;
});

async function onMenuSelect(key: string) {
  menu.visible = false;
  const e = menu.entry;
  const g = menu.group;
  if (g && key === "zipGroup") {
    const items = g.entries.map((x) => [x.path, zipName(x)] as [string, string]);
    const p = await docs.zip(items);
    docs.notice = tf("dc.zipDone", { f: p });
    return;
  }
  if (!e) return;
  switch (key) {
    case "open":
      await openDoc(e);
      break;
    case "archive":
      openPicker(
        t("dc.pickCat"),
        docs.categories.map((c) => ({ key: c, label: c })),
        async (cat) => {
          const n = await docs.move([e.path], cat);
          docs.notice = tf("dc.movedDone", { n });
        },
      );
      break;
    case "aiOp":
      openPicker(t("dc.pickOp"), AI_OPS.value.map((o) => ({ key: o.key, label: o.label })), (op) =>
        runQuickOp(e, op),
      );
      break;
    case "chat": {
      const content = await api.readTextFile(e.path);
      aiStore.openWith(content.slice(0, 15000));
      break;
    }
    case "copyPath": {
      const root = await api.getVaultRoot();
      void navigator.clipboard.writeText(root + "/" + e.path);
      break;
    }
    case "zip": {
      const p = await docs.zip([[e.path, zipName(e)]]);
      docs.notice = tf("dc.zipDone", { f: p });
      break;
    }
    case "ignore":
      await docs.ignore([e.path]);
      break;
  }
}

/** ZIP 包内命名：智能分类=类别/文件名，按目录=相对路径，其余=文件名 */
function zipName(e: DocEntry): string {
  const stem = e.name.replace(/\.md$/i, "");
  if (docs.groupMode === "smart") {
    const cat = e.category;
    return cat && cat !== "未分类" ? `${cat}/${stem}` : stem;
  }
  if (docs.groupMode === "dir") return e.path.replace(/\.md$/i, "");
  if (docs.groupMode === "date") return `${e.date_key}/${stem}`;
  return stem;
}

// ---- 选择器（类别 / AI 操作共用） ----
const picker = reactive({
  visible: false,
  title: "",
  query: "",
  options: [] as { key: string; label: string }[],
  onPick: (_k: string) => {},
});

function openPicker(
  title: string,
  options: { key: string; label: string }[],
  onPick: (key: string) => void,
) {
  picker.title = title;
  picker.query = "";
  picker.options = options;
  picker.onPick = onPick;
  picker.visible = true;
}

const pickerOptions = computed(() => {
  const q = picker.query.trim().toLowerCase();
  return q ? picker.options.filter((o) => o.label.toLowerCase().includes(q)) : picker.options;
});

// ---- AI 批量分类 ----
let unlistenProgress: (() => void) | null = null;

async function runAiClassify() {
  // 分类全部非 AI 目录文档（与插件一致）
  const paths = docs.entries.filter((e) => !e.from_ai_dir).map((e) => e.path);
  if (paths.length === 0) return;
  docs.classify = { done: 0, total: paths.length };
  try {
    const n = await docs.aiClassify(paths);
    docs.notice = tf("dc.classifiedDone", { n });
  } catch (err) {
    docs.notice = String(err);
  } finally {
    docs.classify = null;
  }
}

// ---- AGENTS.md ----
async function runAgents() {
  try {
    const rel = await docs.generateAgents();
    docs.notice = tf("dc.agentsDone", { f: rel });
    await docs.scan();
  } catch (err) {
    docs.notice = String(err);
  }
}

// ---- AI 快捷操作结果 ----
const result = reactive({
  visible: false,
  text: "",
  path: "",
  docName: "",
  opLabel: "",
});

async function runQuickOp(e: DocEntry, op: string) {
  const label = AI_OPS.value.find((o) => o.key === op)?.label ?? op;
  result.visible = true;
  result.text = "…";
  result.path = e.path;
  result.docName = e.name;
  result.opLabel = label;
  try {
    result.text = await docs.quickOp(e.path, op);
  } catch (err) {
    result.text = String(err);
  }
}

async function copyResult() {
  await navigator.clipboard.writeText(result.text);
  result.visible = false;
}

async function appendResult() {
  const content = await api.readTextFile(result.path);
  const stamp = new Date().toLocaleString();
  const merged = `${content.replace(/\s+$/, "")}\n\n---\n\n## AI ${result.opLabel} · ${stamp}\n\n${result.text.trim()}\n`;
  await api.writeTextFile(result.path, merged);
  result.visible = false;
  await docs.scanSoon();
}

async function saveAsNew() {
  const seg = result.path.split("/");
  const name = seg.pop() ?? "doc.md";
  const dir = seg.join("/");
  const stem = name.replace(/\.md$/i, "");
  let target = dir ? `${dir}/${stem}-ai.md` : `${stem}-ai.md`;
  if (await api.pathExists(target)) {
    const stamp = new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "");
    target = dir ? `${dir}/${stem}-ai-${stamp}.md` : `${stem}-ai-${stamp}.md`;
  }
  await api.writeTextFile(target, result.text.trim() + "\n");
  result.visible = false;
  await docs.scanSoon();
  await editor.openNote(target);
}

// ---- 设置区编辑 ----
watch(
  () => [docs.knowledgeDirs, docs.aiToolDirs, docs.showAiToolDocs],
  () => {
    knowledgeEdit.value = docs.knowledgeDirs.map((kd) => [kd[0], kd[1]] as [string, string]);
    aiDirsEdit.value = docs.aiToolDirs;
    aiDocsToggle.value = docs.showAiToolDocs;
  },
  { immediate: true },
);

async function saveKnowledge() {
  await docs.updateKnowledgeDirs(knowledgeEdit.value.map((kd) => [kd[0], kd[1]] as [string, string]));
  await docs.scan();
}

async function saveAiDirs() {
  await docs.updateAiDirs(aiDirsEdit.value);
  await docs.scan();
}

// ---- 生命周期 ----
onMounted(async () => {
  await docs.scan();
  // 首个分组默认展开
  if (groups.value.length > 0) expanded.value = new Set([groups.value[0].key]);
  unlistenProgress = await api.onDocsClassifyProgress((p) => {
    if (docs.classify) docs.classify = p;
  });
  // md 文件变化 → 防抖重扫
  unlistenVault = await api.onVaultChanged((payload) => {
    if (payload.updated.length > 0 || payload.removed.length > 0) docs.scanSoon();
  });
});

let unlistenVault: (() => void) | null = null;

onUnmounted(() => {
  unlistenProgress?.();
  unlistenVault?.();
});
</script>

<style scoped>
.docs-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.dc-count {
  margin-left: auto;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.dc-toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.dc-tool-btn {
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  transition: background var(--anim-fast), color var(--anim-fast);
}
.dc-tool-btn:hover:not(:disabled),
.dc-tool-btn.is-active {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.dc-tool-btn:disabled {
  opacity: 0.5;
}
.is-spin {
  animation: dc-spin 1s linear infinite;
}
@keyframes dc-spin {
  to {
    transform: rotate(360deg);
  }
}
.dc-group-select {
  flex: 1;
  min-width: 0;
  height: 26px;
  font-size: var(--font-ui-size);
}
.dc-progress,
.dc-notice {
  padding: 4px 10px;
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.dc-notice {
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--text-muted);
  cursor: pointer;
}
.dc-notice-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}
.dc-settings {
  padding: 8px 10px;
  border-bottom: 1px solid var(--background-modifier-border);
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
  max-height: 300px;
  overflow-y: auto;
}
.dc-set-title {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.dc-set-grid {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dc-set-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.dc-set-label {
  width: 32px;
  flex-shrink: 0;
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
}
.dc-set-row input {
  flex: 1;
  min-width: 0;
  height: 24px;
}
.dc-set-check {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  cursor: pointer;
}
.dc-set-area {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  resize: vertical;
}
.dc-unignore {
  align-self: flex-start;
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
}
.dc-unignore:hover {
  color: var(--text-normal);
}
.dc-tree {
  flex: 1;
  overflow-y: auto;
  padding: 4px 6px;
}
.dc-group-row {
  gap: 4px;
  font-weight: 600;
}
.dc-group-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dc-group-count {
  margin-left: auto;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.dc-file-row {
  gap: 5px;
}
.dc-fileicon {
  color: var(--text-faint);
  flex-shrink: 0;
}
.dc-file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-ui-size);
}
.dc-file-row.is-active .dc-file-name {
  color: var(--text-accent);
}
.dc-file-dir {
  margin-left: auto;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  flex-shrink: 0;
  max-width: 80px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dc-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 24px 8px;
  font-size: var(--font-ui-size);
}

/* 选择器 / 结果弹窗 */
.dc-picker {
  width: 340px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.dc-picker-title {
  font-weight: 600;
  color: var(--text-normal);
}
.dc-picker-input {
  width: 100%;
}
.dc-picker-list {
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.dc-picker-item {
  text-align: left;
  padding: 6px 10px;
  border-radius: var(--radius-s);
  color: var(--text-normal);
  font-size: var(--font-ui-size);
}
.dc-picker-item:hover {
  background: var(--background-modifier-hover);
}
.dc-picker-cancel {
  align-self: center;
  padding: 4px 16px;
  color: var(--text-faint);
}
.dc-picker-cancel:hover {
  color: var(--text-normal);
}
.dc-result {
  width: 640px;
  height: 480px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.dc-result-title {
  font-weight: 600;
  color: var(--text-normal);
  font-size: var(--font-ui-size);
}
.dc-result-area {
  flex: 1;
  resize: none;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  line-height: 1.6;
}
.dc-result-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
</style>
