<template>
  <div class="ft-children" role="group">
    <div v-for="entry in children" :key="entry.path" class="ft-item">
      <!-- 目录 -->
      <div
        v-if="entry.is_dir"
        class="emd-tree-item ft-row"
        role="treeitem"
        :aria-expanded="vault.expanded[entry.path]"
        :class="{ 'is-dir': true, 'is-build': isBuildDir(entry) }"
        :style="{ paddingLeft: 6 + depth * 14 + 'px' }"
        @click="vault.toggleExpand(entry.path)"
        @contextmenu.prevent="emitMenu($event, entry)"
      >
        <Icon
          :name="vault.expanded[entry.path] ? 'chevron-down' : 'chevron-right'"
          :size="12"
        />
        <span class="ft-name">{{ entry.name }}</span>
      </div>
      <!-- 文件 -->
      <div
        v-else
        class="emd-tree-item ft-row"
        role="treeitem"
        :aria-selected="isActive(entry)"
        :class="{
          'is-active': isActive(entry),
          'is-multi-selected': multiSelected.has(entry.path),
          'git-m': vault.vcsFiles[entry.path] === 'M',
          'git-a': ['A', '?'].includes(vault.vcsFiles[entry.path] ?? ''),
          'git-d': vault.vcsFiles[entry.path] === 'D',
        }"
        :style="{ paddingLeft: 6 + depth * 14 + 'px' }"
        @click="openEntry(entry, $event)"
        @contextmenu.prevent="emitMenu($event, entry)"
      >
        <Icon :name="iconFor(entry)" :size="14" class="ft-fileicon" />
        <span class="ft-name">{{ entry.name }}</span>
        <span v-if="multiSelected.has(entry.path)" class="ft-check-badge"><Icon name="check" :size="12" /></span>
      </div>
      <FileTree
        v-if="entry.is_dir && vault.expanded[entry.path]"
        :dir="entry.path"
        :depth="depth + 1"
        @menu="(e: unknown, entry: FsEntry) => emitMenu(e as MouseEvent, entry)"
        @open="(p: string) => emit('open', p)"
      />
    </div>

    <!-- 展开后无内容/加载失败时给出提示，避免「箭头翻了但什么都没有」被误认为展不开 -->
    <div
      v-if="loadState === 'error'"
      class="ft-hint ft-hint-retry"
      :style="{ paddingLeft: 6 + depth * 14 + 'px' }"
      @click="load"
    >
      {{ t("fe.loadFail") }}
    </div>
    <div
      v-else-if="loadState === 'ok' && children.length === 0"
      class="ft-hint"
      :style="{ paddingLeft: 6 + depth * 14 + 'px' }"
    >
      {{ t("fe.emptyDir") }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import Icon from "../common/Icon.vue";
import { useVaultStore } from "../../stores/vault";
import { useUiStore } from "../../stores/ui";
import { useEditorStore } from "../../stores/editor";
import { useSettingsStore } from "../../stores/settings";
import { BUILD_DIRS, iconForFile, isTextEditable } from "../../lib/filetypes";
import { t } from "../../i18n";
import type { FsEntry } from "../../types";

const props = withDefaults(defineProps<{ dir: string; depth?: number }>(), {
  depth: 0,
});
const emit = defineEmits<{
  menu: [event: MouseEvent, entry: FsEntry];
  open: [path: string];
}>();

const vault = useVaultStore();
const ui = useUiStore();
const editor = useEditorStore();
const children = ref<FsEntry[]>([]);
const multiSelected = ref(new Set<string>());
/** 加载状态：loading/ok/error —— 空目录与加载失败都要有可见反馈 */
const loadState = ref<"loading" | "ok" | "error">("loading");

// 响应父组件的多选集合变化（通过事件同步；组件销毁时移除，防监听器泄漏）
const onMultiSelect = (e: Event) => {
  multiSelected.value = (e as CustomEvent<Set<string>>).detail;
};
window.addEventListener("emd-multi-select-changed", onMultiSelect);
onUnmounted(() => window.removeEventListener("emd-multi-select-changed", onMultiSelect));

async function load() {
  loadState.value = "loading";
  try {
    let list = await vault.loadDir(props.dir);
    const settings = useSettingsStore().data;
    // 代码树：隐藏应用私有目录与笔记专用目录（V3-D6：日记/小计目录分开设定）
    if (props.dir === "") {
      const jotDir = (settings.jot_dir || "jots").replace(/^\/+$|\/+$/g, "");
      const dailyDir = (settings.daily_dir || "daily").replace(/^\/+$|\/+$/g, "");
      list = list.filter((e) => e.name !== jotDir && e.name !== dailyDir && e.name !== ".easyide");
    }
    list = list.filter((e) => e.kind !== "graph");
    children.value = list;
    loadState.value = "ok";
  } catch {
    loadState.value = "error";
  }
}

onMounted(load);

function iconFor(e: FsEntry): string {
  return iconForFile(e.name);
}

function isBuildDir(e: FsEntry): boolean {
  return e.is_dir && BUILD_DIRS.has(e.name);
}

function isActive(e: FsEntry): boolean {
  return e.path === editor.activePath && ui.view === "editor";
}

function openEntry(e: FsEntry, ev?: MouseEvent) {
  // Ctrl+点击 = 多选/取消多选
  if (ev?.ctrlKey || ev?.metaKey) {
    const s = new Set(multiSelected.value);
    if (s.has(e.path)) s.delete(e.path);
    else s.add(e.path);
    multiSelected.value = s;
    window.dispatchEvent(new CustomEvent("emd-multi-select-changed", { detail: s }));
    return;
  }
  // 普通点击：清除多选
  if (multiSelected.value.size > 0) {
    multiSelected.value = new Set();
    window.dispatchEvent(new CustomEvent("emd-multi-select-changed", { detail: new Set() }));
  }
  if (e.kind === "md") {
    if (editor.isDirty) editor.save();
    emit("open", e.path);
    ui.view = "editor";
  } else if (e.kind === "canvas") {
    if (editor.isDirty) editor.save();
    ui.canvasPath = e.path;
    ui.view = "canvas";
  } else if (e.kind === "graph") {
    if (editor.isDirty) editor.save();
    ui.graphPath = e.path;
    ui.view = "graph";
  } else if (e.kind === "image") {
    // 图片走预览弹窗，不进文本编辑链路（V3 修复 B3）
    ui.imagePreview = e.path;
  } else if (e.kind === "other" && isTextEditable(e.path)) {
    // 代码 / 配置等文本文件：进编辑器标签
    if (editor.isDirty) editor.save();
    emit("open", e.path);
    ui.view = "editor";
  }
}

function emitMenu(ev: MouseEvent, entry: FsEntry) {
  // 必须止住冒泡：FileExplorer 根节点也挂了 contextmenu（用于空白处右键），
  // 不拦截的话它会在之后把 entry 覆盖成 null，菜单只剩「新建/导入」三项
  ev.stopPropagation();
  emit("menu", ev, entry);
}
</script>

<style scoped>
.ft-row {
  gap: 5px;
  font-size: var(--font-ui-size);
  transition: background var(--anim-hover), transform var(--anim-press);
}
/* 按压反馈：基座 emd-tree-item 在 anim.css 的全局选择器里，但本组件是 scoped，必须自写 */
.ft-row:active {
  transform: scale(0.98);
}
@media (hover: none) {
  .ft-row:active {
    transform: none;
  }
}
/* 文件夹行底色：与文件行区分 */
.ft-row.is-dir {
  background: var(--background-modifier-hover);
  font-weight: 600;
}
.ft-fileicon {
  color: var(--text-faint);
}
/* 空目录 / 加载失败提示行 */
.ft-hint {
  height: var(--list-item-height);
  display: flex;
  align-items: center;
  font-size: var(--font-ui-smaller);
  color: var(--text-faint);
}
.ft-hint-retry {
  color: var(--text-muted);
  cursor: pointer;
}
.ft-hint-retry:hover {
  color: var(--text-normal);
}
.ft-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 编译产物/依赖目录：置灰弱化 */
.ft-row.is-build {
  opacity: 0.55;
}
/* Git 状态着色：修改橙 / 新增绿 / 删除划线 */
.ft-row.git-m .ft-name {
  color: #e5a03c;
}
.ft-row.git-a .ft-name {
  color: #4ec970;
}
.ft-row.git-d .ft-name {
  text-decoration: line-through;
  opacity: 0.6;
}
/* 多选高亮：紫色背景 + 左侧紫条 + 右侧圆形勾 */
.ft-row.is-multi-selected {
  background: var(--interactive-accent-hover-alt) !important;
  border-left: 3px solid var(--interactive-accent);
  padding-left: 4px !important;
}
.ft-row.is-multi-selected .ft-name {
  color: var(--text-accent);
  font-weight: 600;
}
.ft-check-badge {
  margin-left: auto;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--interactive-accent);
  color: var(--text-on-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
}
</style>
