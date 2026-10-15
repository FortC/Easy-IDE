<template>
  <div v-if="editor.tabs.length > 0" class="etabs">
    <!-- 折叠态：单行紧凑条，只显示当前文件 + 数量（默认，避免长期占高） -->
    <div v-if="collapsed" class="etabs-bar" @click="collapsed = false">
      <Icon :name="iconForFile(editor.activePath)" :size="13" class="etab-icon" />
      <span class="etabs-bar-name">{{ tabLabel(editor.activePath) }}</span>
      <span class="etabs-bar-count">{{ editor.tabs.length }}</span>
      <span class="etabs-bar-hint">{{ t("tab.expand") }}</span>
    </div>

    <!-- 展开态：网格小卡片，一格一个打开的文件；支持拖拽排序 -->
    <div v-else class="etabs-grid" @wheel.prevent="onWheel">
      <div
        v-for="(tab, idx) in editor.tabs"
        :key="tab"
        :ref="(el) => setTabEl(tab, el)"
        class="etab"
        :class="{
          'is-active': tab === editor.activePath,
          'is-dragging': dragFrom === idx,
          'is-drop-before': dropAt === idx && dragFrom !== null && idx < dragFrom,
          'is-drop-after': dropAt === idx && dragFrom !== null && idx > dragFrom,
        }"
        :draggable="true"
        :title="tab"
        @click="editor.activateTab(tab)"
        @auxclick.middle.prevent="editor.closeTab(tab)"
        @contextmenu.prevent="showMenu($event, tab)"
        @dragstart="onDragStart($event, idx)"
        @dragover.prevent="onDragOver($event, idx)"
        @drop.prevent="onDrop($event, idx)"
        @dragend="onDragEnd"
      >
        <Icon :name="iconForFile(tab)" :size="13" class="etab-icon" />
        <span class="etab-name">{{ tabLabel(tab) }}</span>
        <span v-if="editor.tabDirty(tab)" class="etab-dirty" />
        <button class="etab-close" :title="t('tab.close')" @click.stop="editor.closeTab(tab)">
          <Icon name="x" :size="11" />
        </button>
      </div>
    </div>

    <div v-if="editor.tabs.length > 0" class="etabs-actions">
      <button
        class="etabs-collapse"
        :title="collapsed ? t('tab.expand') : t('tab.collapse')"
        @click="collapsed = !collapsed"
      >
        <Icon :name="collapsed ? 'chevrons-right' : 'chevrons-left'" :size="12" />
        <span>{{ collapsed ? t("tab.expand") : t("tab.collapse") }}</span>
      </button>
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
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import DropdownMenu, { type DropItem } from "../common/DropdownMenu.vue";
import { useEditorStore } from "../../stores/editor";
import { iconForFile } from "../../lib/filetypes";
import { tabLabel } from "../../lib/tabLabel";
import { t } from "../../i18n";

const editor = useEditorStore();
const tabEls = new Map<string, HTMLElement>();
/** 标签网格折叠态：默认折叠为单行，编辑区留白（用户可展开成网格扫读） */
const collapsed = ref(true);

const menu = reactive({ visible: false, x: 0, y: 0, path: "" });

function setTabEl(path: string, el: unknown) {
  if (el instanceof HTMLElement) tabEls.set(path, el);
  else tabEls.delete(path);
}

// ---- 拖拽排序 ----
const dragFrom = ref<number | null>(null);
const dropAt = ref<number | null>(null);

function onDragStart(e: DragEvent, idx: number) {
  dragFrom.value = idx;
  // 拖拽期间禁掉过渡，保证跟手（与分隔条拖拽共用同一标记）
  document.body.classList.add("is-resizing");
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    // Firefox 要求设置数据才会触发 drag
    e.dataTransfer.setData("text/plain", String(idx));
  }
}

/** 按指针在卡片左/右半边决定插入位置（网格布局下用横向判定，跨行也直观） */
function onDragOver(e: DragEvent, idx: number) {
  if (dragFrom.value === null) return;
  const el = e.currentTarget as HTMLElement | null;
  if (!el) return;
  const r = el.getBoundingClientRect();
  dropAt.value = e.clientX < r.left + r.width / 2 ? idx : idx + 1;
}

function onDrop(_e: DragEvent, idx: number) {
  const from = dragFrom.value;
  const to = dropAt.value ?? idx;
  if (from !== null) editor.moveTab(from, to);
  clearDrag();
}

function clearDrag() {
  dragFrom.value = null;
  dropAt.value = null;
  document.body.classList.remove("is-resizing");
}

function onDragEnd() {
  clearDrag();
}

const menuItems = computed<DropItem[]>(() => [
  { key: "close", label: t("tab.close"), icon: "x" },
  { key: "closeOthers", label: t("tab.closeOthers") },
  { key: "closeRight", label: t("tab.closeRight") },
  { key: "sep", label: "", separator: true },
  { key: "closeAll", label: t("tab.closeAll") },
]);

function showMenu(ev: MouseEvent, path: string) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.path = path;
  menu.visible = true;
}

async function onMenuSelect(key: string) {
  menu.visible = false;
  const p = menu.path;
  if (key === "close") await editor.closeTab(p);
  else if (key === "closeOthers") await editor.closeOthers(p);
  else if (key === "closeRight") await editor.closeRight(p);
  else if (key === "closeAll") await editor.closeAll();
}

/** 活动标签切换后滚入可视区 */
watch(
  [() => editor.activePath, () => editor.tabs.length, collapsed],
  () => {
    if (collapsed.value) return;
    void nextTick(() => {
      const el = tabEls.get(editor.activePath);
      el?.scrollIntoView({
        behavior: "instant" as ScrollBehavior,
        inline: "nearest",
        block: "nearest",
      });
    });
  },
  { immediate: true },
);

/** 滚轮横向滚动标签网格 */
function onWheel(e: WheelEvent) {
  const grid = document.querySelector(".etabs-grid");
  grid?.scrollBy({ left: e.deltaY + e.deltaX });
}
</script>

<style scoped>
.etabs {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  background: var(--background-secondary);
  border-bottom: 1px solid var(--background-modifier-border);
  min-height: 0;
}

/* ---- 折叠态：单行紧凑条 ---- */
.etabs-bar {
  flex: 1;
  min-width: 0;
  height: 30px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  cursor: pointer;
  color: var(--text-muted);
  transition: background var(--anim-fast);
}
.etabs-bar:hover {
  background: var(--background-modifier-hover);
}
.etabs-bar-name {
  font-size: var(--font-ui-size);
  color: var(--text-normal);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.etabs-bar-count {
  min-width: 16px;
  padding: 0 5px;
  border-radius: 8px;
  background: var(--background-modifier-hover);
  color: var(--text-muted);
  font-size: 10px;
  font-weight: 700;
  text-align: center;
}
.etabs-bar-hint {
  margin-left: auto;
  font-size: var(--font-ui-smaller);
  color: var(--text-faint);
}

/* ---- 展开态：网格小卡片（一格一个文件）---- */
.etabs-grid {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  gap: 4px;
  padding: 5px 6px;
  max-height: 108px;
  overflow-y: auto;
  overflow-x: hidden;
}
.etabs-actions {
  flex-shrink: 0;
  padding-right: 6px;
}
.etabs-collapse {
  height: 22px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  border-radius: var(--radius-s);
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  white-space: nowrap;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.etabs-collapse:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.etabs-collapse:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .etabs-collapse:active {
    transform: none;
  }
}

/* 卡片：所有文件类型统一背景色，靠左侧类型色条区分，靠高亮态区分当前文件 */
.etab {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 6px 0 8px;
  min-width: 0;
  max-width: 132px;
  border-radius: var(--radius-s);
  /* 所有类型统一背景色（用户要求），仅靠图标与左色条识别类型 */
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-left: 2px solid var(--background-modifier-border);
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  cursor: grab;
  cursor: pointer;
  user-select: none;
  position: relative;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press),
    box-shadow var(--anim-hover), opacity var(--anim-fast);
}
.etab:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.etab:active {
  transform: scale(0.97);
}
@media (hover: none) {
  .etab:active {
    transform: none;
  }
}
/* 拖拽排序：源半透明，落点用 inset box-shadow 画插入线（不动 width，守红线2） */
.etab.is-dragging {
  opacity: 0.4;
  cursor: grabbing;
}
.etab.is-drop-before {
  box-shadow: inset 3px 0 0 var(--interactive-accent);
}
.etab.is-drop-after {
  box-shadow: inset -3px 0 0 var(--interactive-accent);
}
/* 拖拽期间全局禁过渡，保证跟手 */
body.is-resizing .etab {
  transition: none;
}
/* 当前编辑文件：高亮（紫色底 + 紫色左条 + 白字） */
.etab.is-active {
  background: var(--interactive-accent-hover-alt);
  border-left-color: var(--interactive-accent);
  color: var(--text-on-accent);
  font-weight: 600;
}
.etab.is-active .etab-icon,
.etab.is-active .etab-close {
  color: var(--text-on-accent);
}
.etab-icon {
  color: var(--text-faint);
  flex-shrink: 0;
}
.etab-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.etab-dirty {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--interactive-accent);
  flex-shrink: 0;
}
.etab.is-active .etab-dirty {
  background: var(--text-on-accent);
}
.etab-close {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-faint);
  flex-shrink: 0;
  opacity: 0;
  transition: opacity var(--anim-fast), background var(--anim-fast);
}
.etab:hover .etab-close,
.etab.is-active .etab-close {
  opacity: 1;
}
.etab-close:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
/* 脏标签：hover 时圆点让位给关闭按钮 */
.etab:hover .etab-dirty {
  display: none;
}
</style>