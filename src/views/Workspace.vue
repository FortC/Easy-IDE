<template>
  <div class="workspace">
    <TitleBar />
    <div class="ws-middle">
      <ActivityBar />

      <!-- 左侧栏：单一面板（语义单一，与右栏视图互不干扰） -->
      <transition name="emd-side-l">
        <div v-if="ui.leftVisible" class="ws-left" :style="{ width: leftW + 'px' }">
          <transition name="emd-pane" mode="out-in">
            <FileExplorer v-if="ui.leftTab === 'code'" />
            <NoteListPanel v-else-if="ui.leftTab === 'notes'" />
            <JotPanel v-else-if="ui.leftTab === 'jots'" />
            <TagPanel v-else-if="ui.leftTab === 'tags'" />
            <component :is="asyncDocs" v-else-if="ui.leftTab === 'docs'" />
            <component :is="asyncVcs" v-else-if="ui.leftTab === 'vcs'" />
            <component :is="asyncMaven" v-else-if="ui.leftTab === 'maven'" />
          </transition>
          <div
            class="ws-resizer"
            role="separator"
            aria-orientation="vertical"
            :aria-valuenow="leftW"
            tabindex="0"
            :title="t('ws.resizeTip')"
            @mousedown="startDrag('left', $event)"
            @dblclick="resetWidth('left')"
            @keydown="onResizerKey('left', $event)"
          />
        </div>
      </transition>

      <!-- 主区域：编辑区为默认视图；canvas / jot 属于「文件类型视图」走主区（随文件打开）；
           图谱 / 日历是左侧大模块入口进来的全幅视图，左右栏保持可见（无布局跳变） -->
      <div class="ws-main">
        <RunBar v-if="ui.showRunBar && ui.view === 'editor'" />
        <transition name="emd-view" mode="out-in">
          <div :key="ui.view" class="ws-main-inner">
            <EditorPane v-if="ui.view === 'editor'" />
            <component :is="asyncGraph" v-else-if="ui.view === 'graph'" />
            <component :is="asyncCalendar" v-else-if="ui.view === 'calendar'" />
            <component :is="asyncCanvas" v-else-if="ui.view === 'canvas'" />
            <component :is="asyncJot" v-else-if="ui.view === 'jot'" />
          </div>
        </transition>
        <ConsolePanel />

        <!-- 右侧栏收起后的展开手柄 -->
        <!-- top 走 CSS 变量，不再硬编码 56px：RunBar 高度变化时无需同步改两处 -->
        <button
          v-if="!ui.rightVisible"
          class="ws-right-expand"
          :style="
            ui.showRunBar && ui.view === 'editor'
              ? { '--expand-top': '48px' }
              : { '--expand-top': '8px' }
          "
          :title="t('ws.expand')"
          @click="ui.rightVisible = true"
        >
          <Icon name="chevron-right" :size="13" />
          <span class="ws-expand-text">{{ t("ws.panel") }}</span>
        </button>
      </div>

      <!-- 右侧栏（可折叠；tab 集随语境自适应） -->
      <transition name="emd-side-r">
        <div v-if="ui.rightVisible" class="ws-right" :style="{ width: rightW + 'px' }">
          <div class="ws-right-tabs">
            <BaseTabs
              :model-value="ui.rightTab"
              :items="rightTabs"
              aria-label="右侧面板"
              @update:model-value="onRightTab($event)"
            />
            <button class="rt-collapse" :title="t('ws.collapse')" @click="ui.rightVisible = false">
              <Icon name="x" :size="13" />
            </button>
          </div>
          <div class="ws-right-body">
            <transition name="emd-pane" mode="out-in">
              <AgentPanel v-if="ui.rightTab === 'ai'" />
              <OutlinePanel v-else-if="ui.rightTab === 'outline'" />
              <!-- 开发语境专属 -->
              <component :is="asyncRefs" v-else-if="ui.rightTab === 'refs'" />
              <component :is="asyncMemory" v-else-if="ui.rightTab === 'memory'" />
              <!-- 笔记语境专属（懒加载：仅 .md 场景使用，连同 YAML 依赖移出首屏） -->
              <component :is="asyncBacklinks" v-else-if="ui.rightTab === 'backlinks'" />
              <component :is="asyncProps" v-else-if="ui.rightTab === 'props'" />
            </transition>
          </div>
          <div
            class="ws-resizer ws-resizer-left"
            role="separator"
            aria-orientation="vertical"
            :aria-valuenow="rightW"
            tabindex="0"
            :title="t('ws.resizeTip')"
            @mousedown="startDrag('right', $event)"
            @dblclick="resetWidth('right')"
            @keydown="onResizerKey('right', $event)"
          />
        </div>
      </transition>
    </div>
    <StatusBar />

    <ToastBar />
    <!-- 四个「按需唤起」的低频面板改懒加载：静态引入会把它们的样式与代码
         全部塞进首屏，是 PF0 首屏 CSS 超标的主要来源。组件内部各自用 store 控制显隐。 -->
    <component :is="asyncIncremental" />
    <component :is="asyncSearch" />
    <component :is="asyncSettings" />
    <NewNoteDialog />
    <ImageViewer />
    <component :is="asyncRecipe" />
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, defineComponent, h, ref, watch } from "vue";
import TitleBar from "../components/common/TitleBar.vue";
import ActivityBar from "../components/common/ActivityBar.vue";
import StatusBar from "../components/common/StatusBar.vue";
import ToastBar from "../components/common/ToastBar.vue";
import NewNoteDialog from "../components/common/NewNoteDialog.vue";
import Icon from "../components/common/Icon.vue";
import BaseTabs from "../components/common/BaseTabs.vue";
import FileExplorer from "../components/sidebar/FileExplorer.vue";
import NoteListPanel from "../components/sidebar/NoteListPanel.vue";
import TagPanel from "../components/sidebar/TagPanel.vue";
import EditorPane from "../components/editor/EditorPane.vue";
import OutlinePanel from "../components/panels/OutlinePanel.vue";
import JotPanel from "../components/sidebar/JotPanel.vue";
import RunBar from "../components/agent/RunBar.vue";
import AgentPanel from "../components/agent/AgentPanel.vue";
import ConsolePanel from "../components/console/ConsolePanel.vue";
import { useUiStore } from "../stores/ui";
import { t } from "../i18n";

// 首屏只加载：编辑区 + 文件树 + AI 面板 + 大纲 + 底部控制台。
// 其余全部懒加载（性能项 PF0/PF5）：首次点击对应入口时才拉取。
// DocsPanel 787 行、VcsPanel、MavenPanel 是重 IO 面板，不该阻塞首屏。
// 懒加载组件的挂起占位：首次点击有解析延迟，没有占位会表现为「点了没反应」。
// 用 h() 而非 template 字符串 —— 前端是 runtime-only 构建，无模板编译器。
const asyncLoading = defineComponent({
  render: () =>
    h("div", { class: "async-loading" }, [
      h("span", { class: "async-loading-dots" }, [h("i"), h("i"), h("i")]),
    ]),
});

const asyncDocs = defineAsyncComponent({ loader: () => import("../components/docs/DocsPanel.vue"), loadingComponent: asyncLoading });
const asyncVcs = defineAsyncComponent({ loader: () => import("../components/vcs/VcsPanel.vue"), loadingComponent: asyncLoading });
const asyncMaven = defineAsyncComponent({ loader: () => import("../components/maven/MavenPanel.vue"), loadingComponent: asyncLoading });
const asyncRefs = defineAsyncComponent({ loader: () => import("../components/panels/RefsPanel.vue"), loadingComponent: asyncLoading });
const asyncMemory = defineAsyncComponent({ loader: () => import("../components/panels/AiMemoryPanel.vue"), loadingComponent: asyncLoading });
// BacklinksPanel / PropertiesPanel 是笔记语境专属（md 双向链接 / frontmatter），
// 代码开发场景永不打开 —— 懒加载后连同 YAML 依赖一并移出首屏主包（PF0）
const asyncBacklinks = defineAsyncComponent({ loader: () => import("../components/panels/BacklinksPanel.vue"), loadingComponent: asyncLoading });
const asyncProps = defineAsyncComponent({ loader: () => import("../components/panels/PropertiesPanel.vue"), loadingComponent: asyncLoading });
// 图谱 / 日历：左侧大模块入口进来的主区全幅视图（force-graph / 日历看板均为重模块，保持懒加载）
const asyncGraph = defineAsyncComponent({ loader: () => import("../components/graph/GraphView.vue"), loadingComponent: asyncLoading });
const asyncCalendar = defineAsyncComponent({ loader: () => import("../components/calendar/CalendarView.vue"), loadingComponent: asyncLoading });
// 文件类型视图（画布 / 小记），随文件打开进入主区
const asyncCanvas = defineAsyncComponent({ loader: () => import("../components/canvas/CanvasEditor.vue"), loadingComponent: asyncLoading });
const asyncJot = defineAsyncComponent({ loader: () => import("../components/jot/JotEditor.vue"), loadingComponent: asyncLoading });
const ImageViewer = defineAsyncComponent({ loader: () => import("../components/editor/ImageViewer.vue"), loadingComponent: asyncLoading });
// 首屏瘦身的四个懒加载面板（PF0）：设置 / 搜索 / 增量包 / 配方卡片
const asyncSettings = defineAsyncComponent({ loader: () => import("../components/settings/SettingsDialog.vue"), loadingComponent: asyncLoading });
const asyncSearch = defineAsyncComponent({ loader: () => import("../components/search/SearchPalette.vue"), loadingComponent: asyncLoading });
const asyncIncremental = defineAsyncComponent({ loader: () => import("../components/pkg/IncrementalDialog.vue"), loadingComponent: asyncLoading });
const asyncRecipe = defineAsyncComponent({ loader: () => import("../components/recipes/RecipeCard.vue"), loadingComponent: asyncLoading });

const ui = useUiStore();

/** 右栏页签（集合由语境派生，见 stores/ui.ts 的 rightTabs getter） */
const rightTabs = computed(() => ui.rightTabs);

/** BaseTabs 只认字符串，这里做一次窄化收口 */
function onRightTab(key: string) {
  ui.setRightTab(key as typeof ui.rightTab);
}

// PF6：栏宽偏好持久化（localStorage，避免每次启动重排；无账号无云依赖）
const WIDTH_KEY = "easyide-panel-widths";
function loadWidths(): { left: number; right: number } {
  try {
    const raw = JSON.parse(localStorage.getItem(WIDTH_KEY) || "{}");
    return {
      left: typeof raw.left === "number" ? raw.left : 268,
      right: typeof raw.right === "number" ? raw.right : 320,
    };
  } catch {
    return { left: 268, right: 320 };
  }
}
const initW = loadWidths();
const leftW = ref(initW.left);
const rightW = ref(initW.right);

watch([leftW, rightW], ([l, r]) => {
  try {
    localStorage.setItem(WIDTH_KEY, JSON.stringify({ left: l, right: r }));
  } catch {
    /* 忽略 */
  }
});

// 语境变化 → 校正右栏 tab（不在新语境集合内则回落 AI，不重置 AI 会话）
watch(
  () => ui.context,
  () => ui.reconcileRightTab(),
);

function startDrag(which: "left" | "right", ev: MouseEvent) {
  ev.preventDefault();
  const startX = ev.clientX;
  const startW = which === "left" ? leftW.value : rightW.value;
  const onMove = (e: MouseEvent) => {
    if (which === "left") {
      leftW.value = Math.min(460, Math.max(220, startW + e.clientX - startX));
    } else {
      rightW.value = Math.min(520, Math.max(220, startW - (e.clientX - startX)));
    }
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    document.body.classList.remove("is-resizing", "is-col-resizing");
  };
  document.body.classList.add("is-resizing", "is-col-resizing");
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

const DEFAULT_W = { left: 268, right: 320 } as const;

/** 双击分隔条 → 复位默认宽度（VSCode/IDEA 习惯） */
function resetWidth(which: "left" | "right") {
  if (which === "left") leftW.value = DEFAULT_W.left;
  else rightW.value = DEFAULT_W.right;
}

/** 键盘调节栏宽：←/→ 每步 10px，Home 复位。方向与拖拽一致（左栏往左变宽） */
function onResizerKey(which: "left" | "right", e: KeyboardEvent) {
  const step = e.shiftKey ? 40 : 10;
  const cur = which === "left" ? leftW.value : rightW.value;
  let next = cur;
  if (e.key === "ArrowLeft") next = cur + (which === "left" ? step : -step);
  else if (e.key === "ArrowRight") next = cur - (which === "left" ? step : -step);
  else if (e.key === "Home") next = DEFAULT_W[which];
  else return;
  e.preventDefault();
  if (which === "left") leftW.value = Math.min(460, Math.max(220, next));
  else rightW.value = Math.min(520, Math.max(220, next));
}
</script>

<style scoped>
/* 左侧栏：宽度用 grid 过渡（合成器友好，不触发 reflow） */
.workspace {
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--background-primary);
}
.ws-middle {
  flex: 1;
  display: flex;
  min-height: 0;
  width: 100%;
}
.ws-left {
  position: relative;
  background: var(--background-secondary);
  border-right: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
  display: flex;
  min-width: 0;
  /* 只在显隐切换时做位移淡入（拖拽调宽时已被 body.is-col-resizing 全局禁动画，保证 1:1 跟手） */
  transition: opacity var(--dur-base) var(--anim-ease-out);
}
.ws-main {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  display: flex;
  /* 纵向布局：工具条在顶、编辑区居中、输出/终端沉底（IDEA 式） */
  flex-direction: column;
  position: relative;
  width: 100%;
}
.ws-main-inner {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  display: flex;
  width: 100%;
}
/* 主区子视图一律撑满并允许收缩 */
.ws-main-inner > * {
  flex: 1 1 auto;
  min-width: 0;
  width: 100%;
}
.ws-right {
  position: relative;
  background: var(--background-secondary);
  border-left: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.ws-right-tabs {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  border-bottom: 1px solid var(--background-modifier-border);
  padding: 0 4px;
  flex-shrink: 0;
}
/* 页签本体已由 BaseTabs 提供（含选中态与按压反馈），此处只管容器与折叠钮 */
.rt-collapse {
  margin-left: auto;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-faint);
}
.rt-collapse {
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.rt-collapse:active {
  transform: scale(0.92);
}
@media (hover: none) {
  .rt-collapse:active {
    transform: none;
  }
}
.rt-collapse:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.ws-right-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.ws-resizer {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 20;
}
.ws-resizer-left {
  right: auto;
  left: -3px;
}

/* 右侧栏收起后的展开手柄（位于头部工具栏下方；top 由 --expand-top 决定，
   RunBar 显隐时由内联样式切变量，避免「RunBar 高度」与常量两处真相源） */
.ws-right-expand {
  position: absolute;
  right: 0;
  top: var(--expand-top, 8px);
  display: flex;
  align-items: center;
  gap: 3px;
  padding: 7px 5px;
  background: var(--background-secondary);
  border: 1px solid var(--background-modifier-border);
  border-right: none;
  border-radius: var(--radius-m) 0 0 var(--radius-m);
  color: var(--text-muted);
  /* 压在 RunBar(z-index:2) 之下，二者不再互相遮挡 */
  z-index: 1;
  transition: background var(--anim-fast), color var(--anim-fast);
}
.ws-right-expand:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.ws-expand-text {
  writing-mode: vertical-rl;
  font-size: 11px;
  letter-spacing: 0.15em;
  line-height: 1;
}
</style>
