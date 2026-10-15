<template>
  <nav class="activitybar" aria-label="主模块导航">
    <!-- 主入口：左栏面板 + 图谱/日历大模块（语义单一，全部是「主区域的大模块」） -->
    <button
      v-for="b in moduleButtons"
      :key="b.key"
      class="ab-btn"
      :aria-pressed="isActive(b)"
      :class="{ 'is-active': isActive(b) }"
      :title="t(b.title)"
      :aria-label="t(b.name)"
      @click="activate(b)"
    >
      <Icon :name="b.icon" :size="19" class="ab-icon" />
      <!-- hover：图标淡出、模块名淡入（只动 opacity/transform） -->
      <span class="ab-label">{{ t(b.name) }}</span>
      <span v-if="isActive(b)" class="ab-indicator" />
    </button>

    <div class="ab-spacer" />

    <div class="ab-sep" />

    <!-- 底部：主题 / 设置 -->
    <button class="ab-btn" :title="t('ab.theme')" @click="settings.toggleTheme()">
      <Icon :name="settings.data.theme === 'dark' ? 'sun' : 'moon'" :size="19" class="ab-icon" />
      <span class="ab-label">{{ t("ab.name.theme") }}</span>
    </button>
    <button class="ab-btn" :title="t('ab.settings')" @click="ui.settingsOpen = true">
      <Icon name="settings" :size="19" class="ab-icon" />
      <span class="ab-label">{{ t("ab.name.settings") }}</span>
    </button>
  </nav>
</template>

<script setup lang="ts">
import { computed } from "vue";
import Icon from "./Icon.vue";
import { useUiStore } from "../../stores/ui";
import { useSettingsStore } from "../../stores/settings";
import { useEditorStore } from "../../stores/editor";
import { t } from "../../i18n";
import type { LeftTab, MainView } from "../../types";

const ui = useUiStore();
const settings = useSettingsStore();
const editor = useEditorStore();

interface ModuleBtn {
  key: LeftTab | "graph" | "calendar";
  icon: string;
  title: string;
  name: string;
  /** panel = 切左栏面板；view = 切主区大视图 */
  kind: "panel" | "view";
}

/**
 * 左栏大模块入口：8 个。
 * 前 6 个切左栏面板（项目结构/文档中心/版本控制/Maven/笔记/标签）；
 * 图谱/日历是主区全幅视图（GraphView/CalendarView 自带完整布局，右栏 320px 装不下），
 * 入口与其他大模块并列，点击后左右栏保持可见。
 */
const moduleButtons: ModuleBtn[] = [
  { key: "code", icon: "folder", title: "ab.code", name: "ab.name.code", kind: "panel" },
  { key: "docs", icon: "book", title: "ab.docs", name: "ab.name.docs", kind: "panel" },
  { key: "vcs", icon: "git-branch", title: "ab.vcs", name: "ab.name.vcs", kind: "panel" },
  { key: "maven", icon: "package", title: "ab.maven", name: "ab.name.maven", kind: "panel" },
  { key: "notes", icon: "file-text", title: "ab.notes", name: "ab.name.notes", kind: "panel" },
  { key: "jots", icon: "pencil", title: "ab.jots", name: "ab.name.jots", kind: "panel" },
  { key: "tags", icon: "tag", title: "ab.tags", name: "ab.name.tags", kind: "panel" },
  { key: "graph", icon: "share-2", title: "ab.graph", name: "ab.name.graph", kind: "view" },
  { key: "calendar", icon: "calendar-days", title: "ab.calendar", name: "ab.name.calendar", kind: "view" },
];

/** 图谱/日历视图激活时，面板按钮不高亮（同一时刻只有一个入口显示激活态） */
const auxViewActive = computed(() => ui.view === "graph" || ui.view === "calendar");

function isActive(b: ModuleBtn): boolean {
  return b.kind === "view" ? ui.view === b.key : ui.leftTab === b.key && !auxViewActive.value;
}

/**
 * 切模块：
 * - 面板：点当前已激活的面板 = 收起左栏（再点展开）；面板与主区视图是两个独立维度，
 *   在图谱视图下打开文件树是合理诉求，故切面板不改主区视图。
 * - 视图：再点当前视图 = 回编辑区。
 */
function activate(b: ModuleBtn) {
  if (b.kind === "view") {
    if (ui.view === b.key) {
      ui.view = "editor";
      return;
    }
    if (editor.isDirty) editor.save();
    ui.view = b.key as MainView;
    return;
  }
  const key = b.key as LeftTab;
  if (ui.leftTab === key && ui.leftVisible && !auxViewActive.value) {
    ui.leftVisible = false;
    return;
  }
  ui.leftTab = key;
  ui.leftVisible = true;
}
</script>

<style scoped>
/* 48px 纯图标 Activity Bar（对齐 AGENTS.md 规范，非 Office Ribbon） */
.activitybar {
  width: var(--activitybar-width);
  min-width: var(--activitybar-width);
  background: var(--background-secondary);
  border-right: 1px solid var(--background-modifier-border);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  padding: 6px 0;
  gap: 2px;
  flex-shrink: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.ab-btn {
  position: relative;
  height: 44px;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  transition: color var(--anim-hover), background var(--anim-hover);
}
/* M-2 按压反馈（scoped 组件必须自带，anim.css 全局选择器不命中） */
.ab-btn:active {
  transform: scale(0.92);
  transition: transform var(--anim-press);
}
.ab-btn:hover {
  color: var(--text-normal);
  background: var(--background-modifier-hover);
}
.ab-btn.is-active {
  color: var(--text-normal);
  background: var(--background-modifier-active-hover);
}
/* hover 交叉淡入淡出：图标淡出、模块名淡入（只动 opacity，红线合规） */
.ab-icon {
  transition: opacity var(--anim-hover);
}
.ab-label {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0;
  white-space: nowrap;
  opacity: 0;
  pointer-events: none;
  transition: opacity var(--anim-hover);
}
.ab-btn:hover .ab-icon {
  opacity: 0;
}
.ab-btn:hover .ab-label {
  opacity: 1;
}
/* 触摸设备无 hover，保持纯图标 */
@media (hover: none) {
  .ab-btn:hover .ab-icon {
    opacity: 1;
  }
  .ab-btn:hover .ab-label {
    opacity: 0;
  }
}
/* 选中指示：左侧 2px 竖条，只动 transform，不触发重排 */
.ab-indicator {
  position: absolute;
  left: 0;
  top: 50%;
  width: 2px;
  height: 20px;
  margin-top: -10px;
  background: var(--interactive-accent);
  animation: ab-indicator-in var(--dur-fast) var(--anim-ease-out);
}
@keyframes ab-indicator-in {
  from {
    opacity: 0;
    transform: scaleY(0.3);
  }
  to {
    opacity: 1;
    transform: scaleY(1);
  }
}
.ab-sep {
  height: 1px;
  margin: 6px 10px;
  background: var(--background-modifier-border);
  flex-shrink: 0;
}
.ab-spacer {
  flex: 1;
  min-height: 12px;
}
</style>
