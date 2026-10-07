<template>
  <div v-if="editor.tabs.length > 0" class="etabs" @wheel.prevent="onWheel">
    <div ref="stripEl" class="etabs-strip">
      <div
        v-for="tab in editor.tabs"
        :key="tab"
        :ref="(el) => setTabEl(tab, el)"
        class="etab"
        :class="{ 'is-active': tab === editor.activePath }"
        :title="tab"
        @click="editor.activateTab(tab)"
        @auxclick.middle.prevent="editor.closeTab(tab)"
        @contextmenu.prevent="showMenu($event, tab)"
      >
        <Icon :name="iconForFile(tab)" :size="13" class="etab-icon" />
        <span class="etab-name">{{ baseName(tab) }}</span>
        <span v-if="editor.tabDirty(tab)" class="etab-dirty" />
        <button class="etab-close" :title="t('tab.close')" @click.stop="editor.closeTab(tab)">
          <Icon name="x" :size="12" />
        </button>
      </div>
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
import { t } from "../../i18n";

const editor = useEditorStore();
const stripEl = ref<HTMLElement>();
const tabEls = new Map<string, HTMLElement>();

const menu = reactive({ visible: false, x: 0, y: 0, path: "" });

function setTabEl(path: string, el: unknown) {
  if (el instanceof HTMLElement) tabEls.set(path, el);
  else tabEls.delete(path);
}

function baseName(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
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
  [() => editor.activePath, () => editor.tabs.length],
  () => {
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

/** 滚轮横向滚动标签条 */
function onWheel(e: WheelEvent) {
  stripEl.value?.scrollBy({ left: e.deltaY + e.deltaX });
}
</script>

<style scoped>
.etabs {
  flex-shrink: 0;
  background: var(--background-secondary);
  border-bottom: 1px solid var(--background-modifier-border);
  overflow: hidden;
}
.etabs-strip {
  display: flex;
  align-items: stretch;
  height: 34px;
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: none;
}
.etabs-strip::-webkit-scrollbar {
  display: none;
}
.etab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px 0 10px;
  min-width: 0;
  max-width: 190px;
  flex-shrink: 0;
  border-right: 1px solid var(--background-modifier-border);
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  cursor: pointer;
  user-select: none;
  position: relative;
  transition: background var(--anim-fast), color var(--anim-fast);
}
.etab:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.etab.is-active {
  background: var(--background-primary);
  color: var(--text-normal);
}
.etab.is-active::before {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  height: 2px;
  background: var(--interactive-accent);
}
.etab-icon {
  color: var(--text-faint);
  flex-shrink: 0;
}
.etab.is-active .etab-icon {
  color: var(--interactive-accent);
}
.etab-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.etab-dirty {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--interactive-accent);
  flex-shrink: 0;
}
.etab-close {
  width: 18px;
  height: 18px;
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
/* 脏标签：关闭按钮被圆点顶到 hover 才出现，二者共存时圆点让位 */
.etab:hover .etab-dirty {
  display: none;
}
</style>
