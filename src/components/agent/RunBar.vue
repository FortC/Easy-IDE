<template>
  <div class="runbar">
    <!-- 已保存的动作按钮（B5.2）：点一下直接执行对应命令，不再询问 AI。
         数据来自 .easyide/actions.json，在 AI 对话里「⚡ 保存为按钮」生成。 -->
    <div class="rb-group">
      <button
        v-for="a in saved.ordered"
        :key="a.id"
        class="rb-preset rb-saved"
        :class="stateClass(a)"
        :title="runTitle(a)"
        @click="onSavedClick(a)"
        @contextmenu.prevent="openMenu(a, $event)"
      >
        <Icon :name="a.icon" :size="13" />
        <span>{{ a.label }}</span>
        <!-- 已就绪：绿色常亮点 + pid -->
        <span v-if="isReady(a)" class="rb-live-dot" />
        <!-- 启动中：呼吸点（B2 的「能看出它在启动」） -->
        <span v-else-if="isRunning(a)" class="rb-blink-dot" />
      </button>

      <!-- 该动作正在跑时，行尾出现显式停止按钮（B1：停止进程 ≠ 停止 AI） -->
      <button
        v-if="activeRun"
        class="rb-stop"
        :class="{ 'is-ready': isCurrentReady }"
        :title="t('rb.stopTitle')"
        @click="stopActive"
      >
        <Icon name="square" :size="12" />
        <span>{{ isCurrentReady ? t("rb.restart") : t("rb.stop") }}</span>
      </button>

      <button
        v-if="!saved.ordered.length"
        class="rb-empty-hint"
        :title="t('rb.emptyHint')"
        @click="agent.openPanel()"
      >
        {{ t("rb.empty") }}
      </button>
    </div>

    <div class="rb-right">
      <!-- B6：热部署开关。开着且已接管进程时显示为绿点，让用户知道「改代码会自动重启」 -->
      <button
        class="rb-preset rb-hot"
        :class="{ 'is-on': hot.enabled && hot.managedCount > 0, 'is-busy': hot.compiling }"
        :title="hotTitle"
        @click="toggleHot"
      >
        <Icon name="refresh-cw" :size="13" :class="{ 'is-spin': hot.compiling }" />
        <span>{{ t("hr.short") }}</span>
        <span v-if="hot.compiling" class="rb-blink-dot" />
      </button>
      <!-- 内置动作收进「＋」菜单：需求要求「默认就保留 AI 对话框这里的命令」 -->
      <button class="rb-preset rb-more" :title="t('rb.more')" @click="moreOpen = !moreOpen">
        <Icon name="plus" :size="13" />
      </button>
      <button class="rb-preset rb-pkg" :title="t('pk.title')" @click="ui.pkgOpen = true">
        <Icon name="archive" :size="13" />
        <span>{{ t("pk.short") }}</span>
      </button>
      <button
        class="rb-console-btn"
        :class="{ 'has-running': consoleStore.running.length > 0 }"
        :title="t('cs.termBtnTitle')"
        @click="switchConsole()"
      >
        <Icon name="terminal" :size="13" />
        <span>{{ t("cs.tabTerminal") }}</span>
        <span v-if="consoleStore.running.length" class="rb-count">
          {{ consoleStore.running.length }}
        </span>
      </button>
    </div>

    <!-- 内置动作下拉（run/debug/build… 走 AI 推理） -->
    <div v-if="moreOpen" class="rb-menu" @mousedown.stop>
      <button v-for="a in builtin" :key="a.key" class="rb-menu-row" @click="onBuiltin(a.key)">
        <Icon :name="a.icon" :size="13" />
        <span>{{ t(a.label) }}</span>
        <span v-if="recipeBadge(a.key)" class="rb-menu-badge">{{ t("rb.recipe") }}</span>
      </button>
    </div>

    <!-- 按钮右键菜单：重命名 / 停止关联进程 / 删除 -->
    <div v-if="menuFor" class="rb-menu rb-menu-r" :style="menuPos" @mousedown.stop>
      <button class="rb-menu-row" @click="menuFor = null">
        <Icon name="pencil" :size="13" /><span>{{ t("rb.rename") }}</span>
      </button>
      <button
        v-if="activeRun && menuFor.actionKey"
        class="rb-menu-row"
        @click="stopActive(); menuFor = null"
      >
        <Icon name="square" :size="13" /><span>{{ t("rb.stop") }}</span>
      </button>
      <button class="rb-menu-row is-danger" @click="removeSaved(menuFor.id)">
        <Icon name="eraser" :size="13" /><span>{{ t("rb.delete") }}</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import Icon from "../common/Icon.vue";
import { useAgentStore } from "../../stores/agent";
import { useUiStore } from "../../stores/ui";
import { useEditorStore } from "../../stores/editor";
import { useConsoleStore, type ConsoleRun } from "../../stores/console";
import { useRecipeStore } from "../../stores/recipe";
import { useProjectStore } from "../../stores/project";
import { useSavedActionsStore, type SavedAction } from "../../stores/savedActions";
import { useHotReloadStore } from "../../stores/hotReload";
import {
  DEV_ACTIONS,
  devPrompt,
  staleNoteOf,
  RECIPE_ACTIONS,
  type DevAction,
} from "../../stores/actions";
import { t, tf } from "../../i18n";

const agent = useAgentStore();
const ui = useUiStore();
const editor = useEditorStore();
const consoleStore = useConsoleStore();
const recipeStore = useRecipeStore();
const projectStore = useProjectStore();
const saved = useSavedActionsStore();
const hot = useHotReloadStore();

/** 标题要说清「它到底做了什么」，避免用户误以为是字节码热替换 */
const hotTitle = computed(() => {
  if (hot.compiling) return t("hr.compiling");
  if (!hot.enabled) return t("hr.offTip");
  if (hot.managedCount > 0) return tf("hr.onTip", { n: hot.managedCount });
  return t("hr.idleTip");
});

function toggleHot() {
  hot.enabled = !hot.enabled;
  if (!hot.enabled) hot.dispose();
  ui.notify(hot.enabled ? t("hr.enabled") : t("hr.disabled"), "success");
}

onMounted(() => {
  void projectStore.refresh();
  void recipeStore.refresh();
  void saved.load();
});
// 点空白处收起菜单
onUnmounted(() => {
  window.removeEventListener("mousedown", closeMenus);
});

/** 内置动作（走 AI 推理）默认收进「＋」菜单，不占用头部空间 */
const builtin = computed(() => DEV_ACTIONS);
const moreOpen = ref(false);
const menuFor = ref<SavedAction | null>(null);
const menuPos = ref({ left: "0px", top: "30px" });

function closeMenus() {
  moreOpen.value = false;
  menuFor.value = null;
}
window.addEventListener("mousedown", closeMenus);

/** 找到当前有进程在跑的按钮（取 order 最小的那个，保证「停止」按钮位置稳定） */
const activeRun = computed<ConsoleRun | null>(() => {
  for (const a of saved.ordered) {
    const key = a.actionKey ?? `btn:${a.id}`;
    const r = consoleStore.activeOf(key);
    if (r) return r;
  }
  // 按钮没绑 actionKey 时用按钮自身 id 再找一次
  for (const a of saved.ordered) {
    const r = consoleStore.activeOf(`btn:${a.id}`);
    if (r) return r;
  }
  return null;
});

function runKeyOf(a: SavedAction): string {
  return a.actionKey ?? `btn:${a.id}`;
}
function isRunning(a: SavedAction): boolean {
  return consoleStore.activeOf(runKeyOf(a))?.status === "running";
}
function isReady(a: SavedAction): boolean {
  return consoleStore.activeOf(runKeyOf(a))?.status === "ready";
}
const isCurrentReady = computed(() => activeRun.value?.status === "ready");

function stateClass(a: SavedAction): Record<string, boolean> {
  return {
    "is-running": isRunning(a),
    "is-ready": isReady(a),
  };
}

/** hover 提示：显示实际命令与运行状态（B2 的信息可见性） */
function runTitle(a: SavedAction): string {
  const r = consoleStore.activeOf(runKeyOf(a));
  const bits: string[] = [];
  if (a.kind === "prompt") bits.push(a.prompt ?? "");
  else bits.push(a.command ?? "");
  if (r) {
    if (r.status === "ready") bits.push(tf("rb.readyTip", { pid: r.pid }));
    else if (r.status === "running") bits.push(tf("rb.startingTip", { pid: r.pid }));
  } else {
    const last = consoleStore.lastOf(runKeyOf(a));
    if (last && last.status !== "running") {
      bits.push(tf("rb.lastExit", { code: last.code ?? "-" }));
    }
  }
  return bits.filter(Boolean).join("\n");
}

async function onSavedClick(a: SavedAction) {
  const r = await saved.execute(a);
  if (!r.ok) ui.notify(r.message, "error");
  // actionKey 为空的老数据：把运行记录归到按钮自身名下，状态才对得上
  const run = consoleStore.activeOf(runKeyOf(a));
  if (run && !run.action) run.action = runKeyOf(a);
}

function stopActive() {
  if (activeRun.value) consoleStore.stop(activeRun.value.id);
}

/** 内置动作：走配方快速通道 → 回落 AI 推理 */
async function onBuiltin(key: string) {
  moreOpen.value = false;
  const k = key as DevAction;
  if (RECIPE_ACTIONS.includes(k)) {
    const handled = await recipeStore.runFast(k);
    if (handled) return;
  }
  const prompt = devPrompt(k, {
    file: editor.activePath,
    consoleTail: consoleStore.tail,
    staleNote: staleNoteOf(k, (kk) => recipeStore.statusOf(kk)),
  });
  agent.openPanel();
  await agent.run(prompt);
  if (RECIPE_ACTIONS.includes(k) && agent.lastCommandLog.length > 0) {
    recipeStore.propose(k, agent.lastCommandLog, { conversationId: agent.sessionId, prompt });
  }
}

function openMenu(a: SavedAction, e: MouseEvent) {
  e.preventDefault();
  const host = (e.currentTarget as HTMLElement).getBoundingClientRect();
  menuPos.value = { left: host.left + "px", top: host.bottom + 2 + "px" };
  menuFor.value = a;
}

async function removeSaved(id: string) {
  menuFor.value = null;
  const run = consoleStore.activeOf(`btn:${id}`);
  if (run) consoleStore.stop(run.id);
  await saved.remove(id);
}

function recipeBadge(key: string): boolean {
  if (!RECIPE_ACTIONS.includes(key as DevAction)) return false;
  const st = recipeStore.statusOf(key as DevAction);
  return !!st?.valid;
}

/** 切换控制台（已展开则收起；展开时直落终端页签） */
function switchConsole() {
  if (ui.consoleVisible) {
    ui.consoleVisible = false;
  } else {
    ui.consoleTab = "terminal";
    ui.consoleVisible = true;
  }
}
</script>

<style scoped>
.runbar {
  height: 34px;
  flex-shrink: 0;
  display: flex;
  align-items: stretch;
  gap: 8px;
  padding: 0 8px;
  background: var(--background-secondary);
  border-bottom: 1px solid var(--background-modifier-border);
  /* 关键（B5.3）：作为 column flex 子项必须显式允许收缩 + 裁剪 */
  min-width: 0;
  width: 100%;
  overflow: hidden;
  position: relative;
  z-index: 2;
}
.rb-group {
  display: flex;
  align-items: center;
  gap: 2px;
  /* 没有 min-width:0 时 flex item 默认 min-width:auto，overflow-x 永不触发 */
  min-width: 0;
  flex: 1 1 auto;
  overflow-x: auto;
  scrollbar-width: none;
}
.rb-group::-webkit-scrollbar {
  display: none;
}
.rb-preset {
  height: 24px;
  padding: 0 9px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  white-space: nowrap;
  flex-shrink: 0;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.rb-preset:hover:not(:disabled) {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.rb-preset:active:not(:disabled) {
  transform: scale(0.96);
}
@media (hover: none) {
  .rb-preset:active:not(:disabled) {
    transform: none;
  }
}
.rb-preset:disabled {
  opacity: 0.45;
}

/* ---- B2 三态：idle 常态 / running 启动中 / ready 已就绪 ---- */
.rb-saved.is-running {
  color: var(--text-success);
  background: var(--background-modifier-hover);
}
.rb-saved.is-ready {
  color: var(--text-success);
  box-shadow: inset 0 -2px 0 var(--text-success);
}
.rb-live-dot,
.rb-blink-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--text-success);
  flex-shrink: 0;
}
/* 启动中：呼吸点（只动 opacity，走合成器 —— 动效红线1） */
.rb-blink-dot {
  animation: rb-blink var(--anim-loop) infinite;
}
@keyframes rb-blink {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.25;
  }
}
/* 尊重系统减弱动效设置 */
@media (prefers-reduced-motion: reduce) {
  .rb-blink-dot {
    animation: none;
    opacity: 0.6;
  }
}

/* ---- 停止 / 重启 ---- */
.rb-stop {
  height: 24px;
  padding: 0 9px;
  margin-left: 4px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
  background: transparent;
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  white-space: nowrap;
  flex-shrink: 0;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.rb-stop:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.rb-stop:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .rb-stop:active {
    transform: none;
  }
}
.rb-stop.is-ready {
  color: var(--text-success);
  border-color: var(--text-success);
}

.rb-empty-hint {
  height: 24px;
  padding: 0 9px;
  border: 0;
  background: transparent;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  white-space: nowrap;
  flex-shrink: 0;
  cursor: pointer;
}
.rb-empty-hint:hover {
  color: var(--text-muted);
}

.rb-more {
  padding: 0 7px;
}
/* ---- B6 热部署开关 ---- */
.rb-hot {
  padding: 0 7px;
  gap: 4px;
}
.rb-hot.is-on {
  color: var(--text-success);
}
/* 编译中：底部 inset 描边，不改变盒子尺寸，避免切换时行宽跳动 */
.rb-hot.is-busy {
  box-shadow: inset 0 -1px 0 currentColor;
}
.rb-count {
  min-width: 14px;
  padding: 0 4px;
  border-radius: 7px;
  background: var(--text-success);
  color: var(--background-primary);
  font-size: 10px;
  font-weight: 700;
  text-align: center;
}
/* 终端按钮：与 .rb-preset 同款 inline-flex，避免缺样式导致图标/文字被浏览器默认按钮布局拆成两行 */
.rb-console-btn {
  height: 24px;
  padding: 0 9px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border: 0;
  background: transparent;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  white-space: nowrap;
  flex-shrink: 0;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.rb-console-btn:hover:not(:disabled) {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.rb-console-btn:active:not(:disabled) {
  transform: scale(0.96);
}
@media (hover: none) {
  .rb-console-btn:active:not(:disabled) {
    transform: none;
  }
}
.rb-console-btn.has-running {
  color: var(--text-success);
}

.rb-right {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

/* ---- 下拉菜单 ---- */
.rb-menu {
  position: absolute;
  top: 32px;
  left: 8px;
  z-index: 30;
  min-width: 170px;
  padding: 4px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  background: var(--background-secondary);
  box-shadow: var(--shadow-l2);
  animation: rb-menu-in var(--anim-enter);
}
.rb-menu-r {
  left: auto;
  top: auto;
}
@keyframes rb-menu-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}
.rb-menu-row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 5px 8px;
  border: 0;
  border-radius: var(--radius-s);
  background: transparent;
  color: var(--text-normal);
  font-size: var(--font-ui-size);
  text-align: left;
  cursor: pointer;
  transition: background var(--anim-hover);
}
.rb-menu-row:hover {
  background: var(--background-modifier-hover);
}
.rb-menu-row:active {
  transform: scale(0.98);
}
@media (hover: none) {
  .rb-menu-row:active {
    transform: none;
  }
}
.rb-menu-row.is-danger {
  color: var(--text-error);
}
.rb-menu-badge {
  margin-left: auto;
  font-size: 10px;
  color: var(--text-accent);
}
</style>