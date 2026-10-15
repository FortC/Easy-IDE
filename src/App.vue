<template>
  <VaultPicker v-if="!vault.opened" />
  <Workspace v-else />

  <!-- 系统打开文件时的模式选择弹窗（懒加载：样式不进首屏包） -->
  <transition name="emd-fade">
    <OpenChoiceDialog
      v-if="openChoice.visible"
      :visible="openChoice.visible"
      :file-name="openChoice.fileName"
      @select="doOpen"
    />
  </transition>
</template>

<script setup lang="ts">
import { defineAsyncComponent, onMounted, onUnmounted, ref } from "vue";
import VaultPicker from "./views/VaultPicker.vue";
import Workspace from "./views/Workspace.vue";
import { t } from "./i18n";
import { api } from "./ipc/tauri";
import { useSettingsStore } from "./stores/settings";
import { useVaultStore } from "./stores/vault";
import { useNotesIndexStore } from "./stores/notesIndex";
import { useEditorStore } from "./stores/editor";
import { useUiStore } from "./stores/ui";
import { useSyncStore } from "./stores/sync";
import { useAgentStore } from "./stores/agent";
import { useConsoleStore } from "./stores/console";
import { useHotReloadStore } from "./stores/hotReload";
import { seqFromEvent, useKeybindingsStore, CM_RESERVED_SEQS } from "./stores/keybindings";
import { openDailyNote } from "./lib/actions";

// 系统打开文件弹窗：仅该场景使用，懒加载以把 ~2KB 样式移出首屏 CSS（性能预算）
const OpenChoiceDialog = defineAsyncComponent(
  () => import("./components/common/OpenChoiceDialog.vue"),
);

const settings = useSettingsStore();
const vault = useVaultStore();
const notesIndex = useNotesIndexStore();
const editor = useEditorStore();
const ui = useUiStore();
const sync = useSyncStore();
const agentStore = useAgentStore();
const consoleStore = useConsoleStore();
const hotReload = useHotReloadStore();
const kb = useKeybindingsStore();

onMounted(async () => {
  await settings.init();
  kb.init();
  await sync.init();
  agentStore.bindEvents();
  consoleStore.bindEvents();
  applyCssSnippets();
  window.addEventListener("emd-snippets-changed", applyCssSnippets);

  // 定时同步（settings.sync_interval_minutes > 0 时启用）
  setInterval(() => {
    const min = settings.data.sync_interval_minutes;
    if (min > 0 && vault.opened && !sync.running && settings.data.active_mcp) {
      sync.syncNow();
    }
  }, 60_000);

  // 索引增量事件：镜像更新 + 刷新文件树受影响目录
  api.onVaultChanged((payload) => {
    notesIndex.applyVaultChanged(payload);
    // B6：源码变更交给热部署编排（内部去抖，只排期不阻塞）
    if (payload.code_changed.length > 0) {
      hotReload.onCodeChanged(payload.code_changed);
    }
    if (payload.code_changed.length > 0 || payload.updated.length > 0 || payload.removed.length > 0) {
      vault.refreshVcsSoon();
    }
    for (const rel of [
      ...payload.updated.map((u) => u.path),
      ...payload.removed,
      ...payload.canvas_changed,
      ...payload.graph_changed,
    ]) {
      vault.refreshParents(rel);
    }
    if (payload.canvas_changed.length > 0) {
      for (const c of payload.canvas_changed) {
        window.dispatchEvent(new CustomEvent("emd-canvas-changed", { detail: c }));
      }
    }
    if (payload.graph_changed.length > 0) {
      for (const g of payload.graph_changed) {
        window.dispatchEvent(new CustomEvent("emd-graph-changed", { detail: g }));
      }
    }
  });

  // 系统打开文件：运行中右键/双击 → 事件转发
  api.onOsOpenFile((path) => openOsFile(path));

  // 后台索引完成 → 同步笔记镜像（代码索引在命令层直读，无需镜像）
  void api.onWorkspaceIndexed(async () => {
    try {
      notesIndex.setNotes(await api.getAllNotes());
    } catch {
      /* 索引不可用时静默 */
    }
  });

  // 自动恢复上次 vault（启动参数带文件时优先打开文件）
  const pending = await api.getPendingFile();
  if (pending) {
    await openOsFile(pending);
    return;
  }
  const last = settings.data.last_vault;
  if (last) {
    try {
      await vault.open(last);
    } catch {
      settings.data.last_vault = null;
      await settings.persist();
    }
  }
});

/** 打开模式选择弹窗状态 */
const openChoice = ref<{
  visible: boolean;
  path: string;
  fileName: string;
}>({ visible: false, path: "", fileName: "" });

/** 打开磁盘上的 md/canvas：显示选择弹窗 */
function openOsFile(abs: string) {
  const fileName = abs.replace(/\\/g, "/").split("/").pop() || t("oc.file");
  openChoice.value = { visible: true, path: abs, fileName };
}

/** 用户选择后执行 */
async function doOpen(mode: "vault" | "temp" | "cancel") {
  const abs = openChoice.value.path;
  openChoice.value.visible = false;
  if (mode === "cancel") {
    await fallbackOpen();
    return;
  }
  try {
    const res =
      mode === "vault"
        ? await api.openPathFromOs(abs)
        : await api.openAsTemp(abs);
    vault.applyOpened(res.root, res.notes);
    if (res.rel.toLowerCase().endsWith(".canvas")) {
      ui.canvasPath = res.rel;
      ui.view = "canvas";
    } else {
      await editor.openNote(res.rel);
      ui.view = "editor";
    }
  } catch (e) {
    console.error("打开文件失败：", e);
    await fallbackOpen();
  }
}

async function fallbackOpen() {
  const last = settings.data.last_vault;
  if (last && !vault.opened) {
    try {
      await vault.open(last);
    } catch {
      /* 保持欢迎页 */
    }
  }
}

// ---- 全局快捷键 ----
// 双击 Shift 手势（IDEA 式搜索）：两次纯 Shift 按下、间隔 <400ms 且中间没有其他按键。
// 按住 Shift 产生的 repeat 不计；输入大写字母等「Shift+字母」组合天然打断（字母按键会置脏）。
let lastShiftAt = 0;
let shiftDirty = false;

/**
 * 全局快捷键的事件源判定。
 * - 落在普通表单控件 / 非编辑器 contenteditable 里：交还控件本身，不劫持（否则在 AI 输入框按 Ctrl+P 会弹文件面板）。
 * - 落在 CodeMirror 里：允许触发全局动作，但 CM 自带键位优先（见 CM_RESERVED_SEQS）。
 */
function isPlainFormField(t: EventTarget | null): boolean {
  if (!(t instanceof HTMLElement)) return false;
  const el = t.closest("input, textarea, select, [contenteditable]");
  if (!el) return false;
  return !el.closest(".cm-editor");
}

function inCodeMirror(t: EventTarget | null): boolean {
  return t instanceof HTMLElement && !!t.closest(".cm-editor");
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Shift") {
    if (!e.repeat && !e.ctrlKey && !e.altKey && !e.metaKey) {
      const now = performance.now();
      if (!shiftDirty && lastShiftAt > 0 && now - lastShiftAt < 400) {
        lastShiftAt = 0;
        // 弹窗/命令面板等场景不打断；输入框里也不触发（否则打字时的双击 Shift 会弹搜索）
        if (!isPlainFormField(e.target) && !ui.searchOpen && !ui.settingsOpen && !ui.newNoteOpen && vault.opened) {
          e.preventDefault();
          ui.openSearch("content");
        }
        return;
      }
      lastShiftAt = now;
      shiftDirty = false;
    }
    return;
  }
  shiftDirty = true;
  if (!(e.ctrlKey || e.metaKey)) return;
  // 输入框 / 普通文本域里不劫持快捷键，避免边打字边弹面板
  if (isPlainFormField(e.target)) return;
  const seq = seqFromEvent(e);
  if (!seq) return;
  // 编辑器聚焦时，让位给 CodeMirror 自带键位，避免一次按键触发两个动作
  if (inCodeMirror(e.target) && CM_RESERVED_SEQS.has(seq)) return;
  const action = kb.reverse[seq];
  if (!action) return;
  e.preventDefault();
  switch (action) {
    case "fileSearch":
      ui.openSearch("file");
      break;
    case "classSearch":
      ui.openSearch("class");
      break;
    case "symbolSearch":
      ui.openSearch("symbol");
      break;
    case "contentSearch":
      ui.openSearch("content");
      break;
    case "newNote":
      ui.openNewNote();
      break;
    case "daily":
      openDailyNote();
      break;
    case "cycleMode":
      editor.cycleMode();
      break;
    case "graph":
      if (editor.isDirty) editor.save();
      ui.view = ui.view === "graph" ? "editor" : "graph";
      break;
    case "settings":
      ui.settingsOpen = true;
      break;
    case "save":
      editor.save();
      break;
    case "nextTab":
      editor.cycleTab(1);
      break;
    case "prevTab":
      editor.cycleTab(-1);
      break;
    case "closeTab":
      if (editor.activePath) void editor.closeTab(editor.activePath);
      break;
  }
}

// 首帧就绪耗时（自页面导航起点）：存 localStorage 供「关于」页展示
{
  const t0 = (window as unknown as Record<string, number>)["__easyideBootStart"];
  if (t0 !== undefined) {
    const ms = Math.round(performance.now() - t0);
    try {
      localStorage.setItem("easyide-boot-ms", String(ms));
    } catch {
      /* 忽略 */
    }
    console.info(`[perf] 前端就绪耗时 ${ms}ms`);
  }
}

// PF7：首屏可交互埋点 —— App 的 onMounted 完成时补一条，
// 与 main.ts 的 nav_to_mount 一起构成「导航 → 可交互」完整链路。
onMounted(() => {
  const t0 = (window as unknown as Record<string, number>)["__easyideBootStart"];
  if (t0 === undefined) return;
  const ready = Math.round(performance.now() - t0);
  try {
    const prev = JSON.parse(localStorage.getItem("easyide-boot-breakdown") || "{}");
    prev.app_mounted_ms = ready;
    localStorage.setItem("easyide-boot-breakdown", JSON.stringify(prev));
  } catch {
    /* 忽略 */
  }
});

onMounted(() => window.addEventListener("keydown", onKeydown));
onUnmounted(() => window.removeEventListener("keydown", onKeydown));

/** 应用启用的 CSS 片段（配置目录 snippets/*.css → 注入 <style>） */
async function applyCssSnippets() {
  document.querySelectorAll("style[data-emd-snippet]").forEach((n) => n.remove());
  try {
    const files = await api.snippetFiles();
    // settings 里没有记录的新文件自动补上（默认停用）
    for (const f of files) {
      if (!settings.data.css_snippets.find((s) => s.name === f)) {
        settings.data.css_snippets.push({ name: f, enabled: false });
      }
    }
    settings.data.css_snippets = settings.data.css_snippets.filter((s) =>
      files.includes(s.name),
    );
    for (const s of settings.data.css_snippets) {
      if (!s.enabled) continue;
      const css = await api.readSnippetFile(s.name);
      const style = document.createElement("style");
      style.setAttribute("data-emd-snippet", s.name);
      style.textContent = css;
      document.head.appendChild(style);
    }
    await settings.persist();
  } catch {
    /* 片段目录不可用时静默 */
  }
}
</script>

<style>
#app {
  height: 100%;
}
</style>
