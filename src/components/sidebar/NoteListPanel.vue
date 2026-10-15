<template>
  <div class="nl-panel">
    <div class="emd-panel-header">
      <Icon name="file-text" :size="13" />
      <span>{{ t("ab.notes") }}</span>
      <span class="nl-count">{{ filtered.length }}</span>
      <button class="nl-tool-btn" :title="t('fe.newNote')" @click="ui.openNewNote()">
        <Icon name="plus" :size="13" />
      </button>
    </div>
    <div class="nl-filter">
      <input v-model="filter" type="text" class="nl-filter-input" :placeholder="t('nl.filterPh')" />
    </div>
    <div class="nl-list">
      <div v-for="group in grouped" :key="group.folder" class="nl-group">
        <div class="nl-group-name">{{ group.folder || t("nl.root") }}</div>
        <div
          v-for="n in group.items"
          :key="n.path"
          class="emd-tree-item nl-item"
          :class="{ 'is-active': editor.activePath === n.path }"
          :title="n.path"
          @click="openNote(n)"
          @contextmenu.prevent="showMenu($event, n)"
        >
          <Icon name="file-text" :size="13" class="nl-icon" />
          <span class="nl-title">{{ n.title }}</span>
          <span class="nl-date">{{ fmtDate(n.mtime) }}</span>
        </div>
      </div>
      <div v-if="filtered.length === 0" class="nl-empty">{{ t("nl.empty") }}</div>
    </div>

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
import { computed, reactive, ref } from "vue";
import Icon from "../common/Icon.vue";
import DropdownMenu, { type DropItem } from "../common/DropdownMenu.vue";
import { useEditorStore } from "../../stores/editor";
import { useNotesIndexStore } from "../../stores/notesIndex";
import { useSettingsStore } from "../../stores/settings";
import { useUiStore } from "../../stores/ui";
import { useVaultStore } from "../../stores/vault";
import { api } from "../../ipc/tauri";
import { t, tf } from "../../i18n";
import type { NoteIndex } from "../../types";

const editor = useEditorStore();
const indexStore = useNotesIndexStore();
const settings = useSettingsStore();
const ui = useUiStore();
const vault = useVaultStore();

const filter = ref("");

/** 笔记清单：全部 .md，剔除小计/日记目录（它们各自独立维护，V3） */
const filtered = computed<NoteIndex[]>(() => {
  const jotDir = (settings.data.jot_dir || "jots").replace(/^\/+|\/+$/g, "");
  const dailyDir = (settings.data.daily_dir || "daily").replace(/^\/+|\/+$/g, "");
  const q = filter.value.trim().toLowerCase();
  return indexStore.notes
    .filter((n) => {
      const top = n.path.split("/")[0];
      return top !== jotDir && top !== dailyDir && top !== "jots" && top !== "daily";
    })
    .filter((n) => !q || n.title.toLowerCase().includes(q) || n.path.toLowerCase().includes(q))
    .sort((a, b) => b.mtime - a.mtime);
});

/** 按目录分组（根目录在最前），组内按修改时间倒序 */
const grouped = computed(() => {
  const map = new Map<string, NoteIndex[]>();
  for (const n of filtered.value) {
    const parts = n.path.split("/");
    parts.pop();
    const folder = parts.join("/");
    if (!map.has(folder)) map.set(folder, []);
    map.get(folder)!.push(n);
  }
  return [...map.entries()]
    .sort((a, b) => (a[0] === "" ? -1 : b[0] === "" ? 1 : a[0].localeCompare(b[0])))
    .map(([folder, items]) => ({ folder, items }));
});

function fmtDate(mtime: number): string {
  const d = new Date(mtime * 1000);
  const pad = (x: number) => String(x).padStart(2, "0");
  return `${d.getMonth() + 1}-${pad(d.getDate())}`;
}

async function openNote(n: NoteIndex) {
  if (editor.isDirty) editor.save();
  await editor.openNote(n.path);
  ui.view = "editor";
}

// ---- 右键菜单：打开 / 删除 ----
const menu = reactive({ visible: false, x: 0, y: 0, target: null as NoteIndex | null });

const menuItems = computed<DropItem[]>(() => [
  { key: "open", label: t("dc.open"), icon: "file-text" },
  { key: "sep", label: "", separator: true },
  { key: "delete", label: t("fe.delete"), icon: "trash-2", danger: true },
]);

function showMenu(ev: MouseEvent, n: NoteIndex) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.target = n;
  menu.visible = true;
}

async function onMenuSelect(key: string) {
  menu.visible = false;
  const n = menu.target;
  if (!n) return;
  if (key === "open") {
    await openNote(n);
  } else if (key === "delete") {
    if (!confirm(tf("fe.confirmDel", { name: n.title }))) return;
    try {
      await api.deletePath(n.path);
      editor.removePath(n.path);
      await indexStore.rebuild();
      await vault.refreshParents(n.path);
    } catch (e) {
      alert(String(e));
    }
  }
}
</script>

<style scoped>
.nl-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.nl-count {
  margin-left: auto;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.nl-tool-btn {
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.nl-tool-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.nl-filter {
  padding: 6px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.nl-filter-input {
  width: 100%;
  height: 24px;
  font-size: var(--font-ui-size);
}
.nl-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 6px;
}
.nl-group-name {
  padding: 6px 6px 2px;
  font-size: 10px;
  letter-spacing: 0.06em;
  color: var(--text-faint);
  text-transform: uppercase;
}
.nl-item {
  gap: 6px;
}
.nl-icon {
  color: var(--text-faint);
  flex-shrink: 0;
}
.nl-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-ui-size);
}
.nl-item.is-active .nl-title {
  color: var(--text-accent);
}
.nl-date {
  flex-shrink: 0;
  color: var(--text-faint);
  font-size: 10px;
}
.nl-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 24px 10px;
  font-size: var(--font-ui-size);
  white-space: pre-wrap;
}
</style>
