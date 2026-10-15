<template>
  <div class="ag-panel">
    <!-- 头部：会话管理 -->
    <div class="ag-head">
      <select
        class="ag-session-select"
        :value="agent.sessionId"
        @change="onSessionChange"
      >
        <option value="" disabled>{{ t("ag.pickSession") }}</option>
        <option v-for="s in agent.sessions" :key="s.id" :value="s.id">
          {{ s.title || s.id }} · {{ shortTime(s.updated) }}
        </option>
      </select>
      <button class="ag-tool-btn" :title="t('ag.newSession')" @click="agent.newSession()">
        <Icon name="plus" :size="13" />
      </button>
      <button
        v-if="agent.sessionId"
        class="ag-tool-btn"
        :title="t('ag.delSession')"
        @click="agent.deleteSession(agent.sessionId)"
      >
        <Icon name="trash-2" :size="13" />
      </button>
    </div>

    <!-- 消息流 -->
    <div ref="listEl" class="ag-list">
      <div v-if="agent.items.length === 0" class="ag-placeholder">
        {{ t("ag.ph") }}
      </div>
      <template v-for="(it, i) in agent.items" :key="i">
        <div v-if="it.role === 'user'" class="ag-msg ag-msg-user">
          <div class="ag-bubble ag-bubble-user">{{ it.content }}</div>
        </div>
        <div v-else class="ag-msg ag-msg-ai">
          <!-- 工具调用块：语义化图标 + 步骤序号 + 状态标识（AI6） -->
          <div v-if="it.tool" class="ag-tool" :class="{ 'is-rejected': it.tool.rejected }">
            <button class="ag-tool-head" @click="toggleTool(i)">
              <span class="ag-tool-step">{{ toolStep(i) }}</span>
              <Icon :name="toolIcon(it.tool.name)" :size="12" />
              <span class="ag-tool-name">{{ toolLabel(it.tool.name) }}</span>
              <span class="ag-tool-args">{{ toolSummary(it.tool) }}</span>
              <span class="ag-tool-status" :class="'is-' + (it.tool.rejected ? 'rejected' : 'ok')">
                {{ it.tool.rejected ? t("ag.toolRejected") : t("ag.toolDone") }}
              </span>
              <Icon :name="openTools.has(i) ? 'chevron-down' : 'chevron-right'" :size="12" />
            </button>
            <div v-if="openTools.has(i)" class="ag-tool-body">
              <pre class="ag-pre">{{ it.tool.result }}</pre>
              <!-- B5.2：AI 执行过命令 → 可存成代码区头部的按钮，点击直接执行不再问AI -->
              <div v-if="saveableOf(it.tool)" class="ag-tool-save">
                <button
                  class="ag-save-btn"
                  :class="{ 'is-done': savedIds.has(saveKeyOf(it.tool)) }"
                  :title="t('sa.saveAsTip')"
                  @click="saveAsButton(it.tool)"
                >
                  <Icon name="zap" :size="12" />
                  <span>{{ savedIds.has(saveKeyOf(it.tool)) ? t("sa.saved") : t("sa.saveAs") }}</span>
                </button>
              </div>
            </div>
          </div>
          <div
            v-if="stripToolJson(it.content)"
            class="ag-bubble ag-bubble-ai md-body"
            v-html="renderCached(stripToolJson(it.content))"
          />
        </div>
      </template>

      <!-- 流式输出中 -->
      <div v-if="agent.streaming" class="ag-msg ag-msg-ai ag-msg-stream">
        <!-- 文件修改提案（write_file 确认门） -->
        <div v-if="agent.pendingEdit" class="ag-confirm ag-edit-confirm">
          <div class="ag-confirm-title">{{ t("ag.confirmEdit") }}</div>
          <div class="ag-confirm-path">{{ agent.pendingEdit.path }}</div>
          <div class="ag-confirm-thought">{{ agent.pendingEdit.description }}</div>
          <div class="ag-edit-diff">
            <DiffView :diff="agent.pendingEdit.diff" />
          </div>
          <div class="ag-confirm-actions">
            <button class="emd-btn emd-btn-accent" @click="agent.resolveEdit(true)">
              {{ t("ag.applyEdit") }}
            </button>
            <button class="emd-btn" @click="agent.resolveEdit(false)">
              {{ t("ag.deny") }}
            </button>
          </div>
        </div>
        <div v-else-if="agent.pendingCommand" class="ag-confirm">
          <div class="ag-confirm-title">{{ t("ag.confirmRun") }}</div>
          <div class="ag-confirm-thought">{{ agent.pendingCommand.thought }}</div>
          <pre class="ag-confirm-cmd">{{ agent.pendingCommand.command }}</pre>
          <div class="ag-confirm-actions">
            <button class="emd-btn emd-btn-accent" @click="agent.resolveCommand(true)">
              {{ t("ag.allow") }}
            </button>
            <button class="emd-btn" @click="agent.resolveCommand(false)">
              {{ t("ag.deny") }}
            </button>
          </div>
        </div>
        <div v-else-if="agent.streamText" class="ag-bubble ag-bubble-ai md-body" v-html="streamHtml" />
        <!-- 占位：pendingCommand 分支在上方，保持链完整 -->
        <div v-else class="ag-thinking">
          <span class="ag-dots"><i /><i /><i /></span>
          {{ t("ag.thinking") }}
        </div>
      </div>

      <div v-if="agent.error" class="ag-error">{{ agent.error }}</div>
    </div>

    <!-- 输入区 -->
    <div class="ag-input-area">
      <!-- AI 快捷动作 chips（随语境切换）：AI2 —— 取代原先并行的 RunBar 第二入口 -->
      <div class="ag-chips">
        <button
          v-for="a in actions"
          :key="a.key"
          class="ag-chip"
          :class="{ 'is-recipe': recipeBadge(a.key) }"
          :title="t(a.title)"
          :disabled="agent.streaming"
          @click="runAction(a.key)"
        >
          <Icon :name="a.icon" :size="12" />
          <span>{{ t(a.label) }}</span>
        </button>
      </div>

      <!-- 选区上下文引用（AI5）：选中代码后自动挂载，无需手动复制 -->
      <div v-if="agent.contextSelection" class="ag-ctx">
        <Icon name="file-code" :size="11" />
        <span class="ag-ctx-name">{{ agent.contextSelection.name }}</span>
        <span class="ag-ctx-loc">L{{ agent.contextSelection.from + 1 }}-{{ agent.contextSelection.to + 1 }}</span>
        <button class="ag-ctx-x" :title="t('ag.ctxRemove')" @click="agent.clearSelectionContext()">
          <Icon name="x" :size="10" />
        </button>
      </div>

      <textarea
        v-model="draft"
        class="ag-input"
        :placeholder="t('ag.inputPh')"
        :disabled="agent.streaming"
        rows="3"
        @keydown.ctrl.enter.prevent="send"
        @keydown.meta.enter.prevent="send"
        @keydown.ctrl.i.prevent
      />
      <div class="ag-input-actions">
        <button v-if="agent.streaming" class="emd-btn" @click="agent.stop()">
          <Icon name="square" :size="12" /> {{ t("ag.stop") }}
        </button>
        <button
          v-else
          class="emd-btn emd-btn-accent"
          :disabled="!draft.trim()"
          @click="send"
        >
          <Icon name="play" :size="12" /> {{ t("ag.send") }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import DiffView from "../common/DiffView.vue";
import { useAgentStore, isLongLived } from "../../stores/agent";
import { useUiStore } from "../../stores/ui";
import { useEditorStore } from "../../stores/editor";
import { useConsoleStore } from "../../stores/console";
import { useRecipeStore } from "../../stores/recipe";
import { useSavedActionsStore } from "../../stores/savedActions";
import {
  actionsOf,
  devPrompt,
  notePrompt,
  staleNoteOf,
  RECIPE_ACTIONS,
  type DevAction,
  type NoteAction,
} from "../../stores/actions";
import { t } from "../../i18n";

const agent = useAgentStore();
const ui = useUiStore();
const editor = useEditorStore();
const consoleStore = useConsoleStore();
const recipeStore = useRecipeStore();
const savedActions = useSavedActionsStore();
const draft = ref("");
const listEl = ref<HTMLElement>();
const openTools = ref(new Set<number>());
/** 已存过按钮的命令（用于把「保存为按钮」按钮切成「已保存」态） */
const savedIds = ref(new Set<string>());

/** 只有 run_command 且带实际命令时才可存（B5.2） */
function saveableOf(tool: { name: string; args: Record<string, string> }) {
  if (tool.name !== "run_command") return false;
  const c = tool.args?.command;
  return !!c && c.trim().length > 0;
}
function saveKeyOf(tool: { args: Record<string, string> }): string {
  return (tool.args?.command ?? "").trim();
}

/**
 * 保存为按钮（B5.2）：把这条命令存成代码区头部的按钮。
 * 长驻命令（启动服务类）默认标记 background=true，点击后 RunBar 会显示停止按钮。
 */
async function saveAsButton(tool: { args: Record<string, string> }) {
  const command = (tool.args?.command ?? "").trim();
  if (!command) return;
  // 就绪检测：复用 agent 侧同一套正则思路，这里只存最通用的一条
  const readyPattern = /port|端口/i.test(command)
    ? "Started .* on port \\d+|listening on port \\d+|Local:\\s+http"
    : undefined;
  await savedActions.add({
    label: guessLabel(command),
    kind: "command",
    command,
    background: isLongLived(command),
    readyPattern,
    actionKey: isLongLived(command) ? "start" : undefined,
  });
  savedIds.value = new Set(savedIds.value).add(command);
  ui.notify(t("sa.saved"));
}

/** 从命令里猜一个短按钮名（去掉参数噪音） */
function guessLabel(command: string): string {
  const c = command.trim();
  if (/spring-boot:run|bootRun|boot:run/.test(c)) return "启动";
  if (/npm run dev|pnpm dev|vite/.test(c)) return "前端启动";
  if (/mvn .*compile/.test(c)) return "编译";
  if (/mvn .*test|gradle test/.test(c)) return "测试";
  if (/java -jar/.test(c)) return "运行";
  if (/clean/.test(c)) return "清理";
  if (/package|install/.test(c)) return "打包";
  // 兜底：取首段命令名
  return c.split(/\s+/)[0] ?? c.slice(0, 12);
}

/** AI 快捷动作 chips（随语境切换） */
const actions = computed(() => actionsOf(ui.context));

/** 配方状态徽标：命中配方显示「直通」态 */
function recipeBadge(key: string): "active" | "stale" | null {
  if (!RECIPE_ACTIONS.includes(key as DevAction)) return null;
  const st = recipeStore.statusOf(key as DevAction);
  if (!st) return null;
  return st.valid ? "active" : "stale";
}

/** 触发快捷动作：代码语境走配方快速通道，笔记语境直接发起会话 */
async function runAction(key: string) {
  if (agent.streaming) return;
  if (ui.context === "note") {
    const sel = agent.contextSelection?.text ?? "";
    const prompt = notePrompt(key as NoteAction, {
      title: editor.activePath.split("/").pop() ?? "",
      content: editor.content,
      selection: sel,
    });
    await agent.run(prompt);
    return;
  }
  // 代码语境：与 RunBar 同源逻辑（配方优先，失败回落 AI 推理）
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

function send() {
  const text = draft.value.trim();
  if (!text || agent.streaming) return;
  draft.value = "";
  void agent.run(text);
}

async function onSessionChange(e: Event) {
  const id = (e.target as HTMLSelectElement).value;
  if (id) await agent.loadSession(id);
}

function toggleTool(i: number) {
  const s = new Set(openTools.value);
  if (s.has(i)) s.delete(i);
  else s.add(i);
  openTools.value = s;
}

function toolSummary(tool: { name: string; args: Record<string, string> }): string {
  const a = tool.args.command || tool.args.path || tool.args.query || "";
  return a.length > 40 ? a.slice(0, 40) + "…" : a;
}

/** 工具名 → 语义化图标（AI6：让调用过程一眼可辨类型） */
function toolIcon(name: string): string {
  switch (name) {
    case "read_file":
      return "file-text";
    case "write_file":
      return "file-code";
    case "list_dir":
      return "folder";
    case "search_text":
    case "search_symbols":
      return "search";
    case "run_command":
      return "terminal";
    default:
      return "settings-2";
  }
}

/** 工具名 → 中文标签 */
function toolLabel(name: string): string {
  const map: Record<string, string> = {
    read_file: "读取文件",
    write_file: "写入文件",
    list_dir: "浏览目录",
    search_text: "全文搜索",
    search_symbols: "符号搜索",
    run_command: "执行命令",
  };
  return map[name] ?? name;
}

/** 步骤序号：该条消息在本次会话的工具调用中的次序 */
function toolStep(i: number): number {
  let n = 0;
  for (let k = 0; k <= i && k < agent.items.length; k++) {
    if (agent.items[k].tool) n++;
  }
  return n;
}

/** 去掉工具 JSON 块后的正文（纯 JSON 工具调用则不显示气泡） */
function stripToolJson(content: string): string {
  const fence = /```(?:json)?\s*[\s\S]*?```/.exec(content);
  if (fence) {
    const rest = content.replace(fence[0], "").trim();
    return rest;
  }
  if (content.trim().startsWith('{"tool"')) {
    return "";
  }
  return content;
}

// 性能：markdown-it 走懒加载 —— AI 消息渲染时才载入，首屏主包不含它。
// v-html 需要同步字符串，故用 Map 缓存渲染结果（载入完成前回落纯文本）。
const htmlCache = reactive(new Map<string, string>());

type MdModule = typeof import("../../lib/markdown/md-render");
const mdMod = ref<MdModule | null>(null);
let mdLoading: Promise<MdModule> | null = null;

/** 懒载入 markdown-it；失败返回 null，调用方回落纯文本 */
async function ensureMd(): Promise<MdModule | null> {
  if (mdMod.value) return mdMod.value;
  if (!mdLoading) {
    mdLoading = import("../../lib/markdown/md-render").then((m) => {
      mdMod.value = m;
      return m;
    });
  }
  try {
    return await mdLoading;
  } catch {
    return null;
  }
}

/** 同步取渲染结果：命中缓存直接返回，否则先返回原文并触发一次异步渲染 */
function renderCached(src: string): string {
  const hit = htmlCache.get(src);
  if (hit !== undefined) return hit;
  void ensureMd().then((m) => {
    if (!m || htmlCache.has(src)) return;
    try {
      htmlCache.set(src, m.renderMarkdown(src, { resolveNote: () => null, attachmentsDir: "assets" }));
    } catch {
      /* 渲染失败保留纯文本 */
    }
  });
  return src;
}

/** 预热：面板挂载后空闲载入 markdown-it */
onMounted(() => void ensureMd());

// 流式输出：单独缓存，避免每帧触发 Map 查找抖动
const streamHtml = ref("");
watch(
  () => agent.streamText,
  async (v) => {
    if (!v) {
      streamHtml.value = "";
      return;
    }
    const m = await ensureMd();
    if (!m) {
      streamHtml.value = v;
      return;
    }
    try {
      streamHtml.value = m.renderMarkdown(v, { resolveNote: () => null, attachmentsDir: "assets" });
    } catch {
      streamHtml.value = v;
    }
  },
  { immediate: true },
);

function shortTime(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

// 新消息 → 滚到底
watch(
  () => [agent.items.length, agent.streamText, agent.pendingCommand, agent.pendingEdit],
  () => {
    void nextTick(() => {
      listEl.value?.scrollTo({ top: listEl.value.scrollHeight });
    });
  },
);

onMounted(() => {
  void agent.refreshSessions();
  if (!agent.sessionId) agent.newSession();
});
</script>

<style scoped>
.ag-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.ag-head {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.ag-session-select {
  flex: 1;
  min-width: 0;
  height: 26px;
  font-size: var(--font-ui-size);
}
.ag-tool-btn {
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.ag-tool-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.ag-list {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.ag-placeholder {
  color: var(--text-faint);
  text-align: center;
  padding: 24px 8px;
  font-size: var(--font-ui-size);
  line-height: 1.7;
}
.ag-msg {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.ag-msg-user {
  align-items: flex-end;
}
.ag-bubble {
  max-width: 100%;
  border-radius: var(--radius-m);
  padding: 8px 10px;
  font-size: var(--font-ui-size);
  line-height: 1.6;
  overflow-wrap: break-word;
}
.ag-bubble-user {
  background: var(--interactive-accent-hover-alt);
  color: var(--text-normal);
  white-space: pre-wrap;
}
.ag-bubble-ai {
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
}
.ag-bubble-ai :deep(pre) {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  background: var(--background-secondary);
  border-radius: var(--radius-s);
  padding: 8px;
  overflow-x: auto;
}
.ag-bubble-ai :deep(code) {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
}
.ag-bubble-ai :deep(p) {
  margin: 0 0 6px;
}
.ag-bubble-ai :deep(p:last-child) {
  margin-bottom: 0;
}
/* 三点脉冲（替代旋转 spinner：重绘更少、视觉噪音更低） */
.ag-dots {
  display: inline-flex;
  gap: 3px;
  align-items: center;
}
.ag-dots i {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--text-faint);
  animation: ag-dot-pulse var(--anim-loop) infinite;
}
.ag-dots i:nth-child(2) {
  animation-delay: 0.18s;
}
.ag-dots i:nth-child(3) {
  animation-delay: 0.36s;
}
@keyframes ag-dot-pulse {
  0%,
  60%,
  100% {
    opacity: 0.3;
    transform: translateY(0);
  }
  30% {
    opacity: 1;
    transform: translateY(-2px);
  }
}
@media (prefers-reduced-motion: reduce) {
  .ag-dots i {
    animation: none;
    opacity: 0.6;
  }
}
.ag-thinking {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-faint);
  font-size: var(--font-ui-size);
  padding: 4px 2px;
}
.ag-error {
  color: var(--text-error);
  font-size: var(--font-ui-smaller);
  padding: 6px 8px;
  border-radius: var(--radius-s);
  background: var(--background-primary);
}

/* 工具调用块 */
.ag-tool {
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  overflow: hidden;
}
.ag-tool.is-rejected {
  opacity: 0.7;
}
.ag-tool-step {
  width: 15px;
  height: 15px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--background-modifier-hover);
  color: var(--text-faint);
  font-size: 9px;
  line-height: 15px;
  text-align: center;
}
.ag-tool-status {
  margin-left: auto;
  padding: 0 5px;
  border-radius: var(--radius-s);
  font-size: 10px;
  line-height: 15px;
  flex-shrink: 0;
}
.ag-tool-status.is-ok {
  background: rgba(63, 185, 80, 0.16);
  color: #3fb950;
}
.ag-tool-status.is-rejected {
  background: rgba(229, 83, 75, 0.16);
  color: var(--text-error);
}
.ag-tool-head {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  background: var(--background-primary);
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
}
.ag-tool-head:hover {
  color: var(--text-normal);
}
.ag-tool-name {
  font-weight: 600;
  color: var(--interactive-accent);
  flex-shrink: 0;
}
.ag-tool-args {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
}
.ag-tool-body {
  border-top: 1px solid var(--background-modifier-border);
  max-height: 220px;
  overflow-y: auto;
}
.ag-pre {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  padding: 8px;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  color: var(--text-muted);
}
/* ---- B5.2「保存为按钮」：把命令固化成代码区头部的按钮 ---- */
.ag-tool-save {
  display: flex;
  justify-content: flex-end;
  padding: 0 8px 8px;
  border-top: 1px solid var(--background-modifier-border);
  padding-top: 6px;
}
.ag-save-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 9px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
  background: transparent;
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  cursor: pointer;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.ag-save-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.ag-save-btn:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .ag-save-btn:active {
    transform: none;
  }
}
.ag-save-btn.is-done {
  color: var(--text-success);
  border-color: var(--text-success);
}

/* 文件修改提案 diff 预览区 */
.ag-edit-confirm {
  max-height: 70vh;
}
.ag-confirm-path {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  color: var(--text-normal);
  background: var(--background-secondary);
  border-radius: var(--radius-s);
  padding: 4px 8px;
}
.ag-edit-diff {
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
  height: 320px;
  overflow: hidden;
}

/* 命令确认门 */
.ag-confirm {
  border: 1px solid var(--interactive-accent);
  border-radius: var(--radius-m);
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--background-primary);
}
.ag-confirm-title {
  font-weight: 600;
  color: var(--text-normal);
}
.ag-confirm-thought {
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
}
.ag-confirm-cmd {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  background: var(--background-secondary);
  border-radius: var(--radius-s);
  padding: 8px;
  white-space: pre-wrap;
  overflow-wrap: break-word;
}
.ag-confirm-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

/* 输入区 */
.ag-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 6px;
}
.ag-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 22px;
  padding: 0 8px;
  border-radius: var(--radius-s);
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  white-space: nowrap;
  transition: background var(--anim-hover), color var(--anim-hover),
    border-color var(--anim-hover), transform var(--anim-press);
}
.ag-chip:hover:not(:disabled) {
  color: var(--text-normal);
  border-color: var(--background-modifier-border-hover);
}
.ag-chip:active:not(:disabled) {
  transform: scale(0.96);
}
.ag-chip:disabled {
  opacity: 0.45;
}
/* 配方命中态（可零 token 直通） */
.ag-chip.is-recipe {
  border-color: var(--interactive-accent);
  color: var(--text-normal);
}
.ag-ctx {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 3px 6px;
  margin-bottom: 6px;
  border-radius: var(--radius-s);
  background: var(--interactive-accent-hover-alt, rgba(139, 108, 239, 0.1));
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  animation: ag-msg-in var(--dur-fast) var(--anim-ease-out);
}
.ag-ctx-name {
  font-family: var(--font-mono);
  color: var(--text-normal);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 45%;
}
.ag-ctx-loc {
  color: var(--text-faint);
}
.ag-ctx-x {
  margin-left: auto;
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-faint);
}
.ag-ctx-x:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.ag-input-area {
  border-top: 1px solid var(--background-modifier-border);
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
}
.ag-input {
  width: 100%;
  resize: none;
  font-size: var(--font-ui-size);
  line-height: 1.5;
}
.ag-input-actions {
  display: flex;
  justify-content: flex-end;
}

/* ========== M-5 AI 面板动效 ========== */
/* 红线7：AI 流式文字不做逐字动画（持续重排必掉帧），只对容器与卡片做入场 */

/* 工具调用结果卡：scale(0.98)→1 + 淡入，180ms */
.ag-tool {
  animation: ag-card-in var(--dur-base) var(--anim-ease-out);
}
@keyframes ag-card-in {
  from {
    opacity: 0;
    transform: scale(0.98);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

/* 命令确认门：必须吸引注意，用滑入而非淡入 */
.ag-confirm {
  animation: ag-confirm-in var(--dur-base) var(--anim-ease-out);
}
@keyframes ag-confirm-in {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 工具执行中：三点脉冲（不旋转 spinner，减少视觉噪音与重绘） */
/* 三点脉冲（替代旋转 spinner：重绘更少、视觉噪音更低） */
.ag-dots {
  display: inline-flex;
  gap: 3px;
  align-items: center;
}
.ag-dots i {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--text-faint);
  animation: ag-dot-pulse var(--anim-loop) infinite;
}
.ag-dots i:nth-child(2) {
  animation-delay: 0.18s;
}
.ag-dots i:nth-child(3) {
  animation-delay: 0.36s;
}
@keyframes ag-dot-pulse {
  0%,
  60%,
  100% {
    opacity: 0.3;
    transform: translateY(0);
  }
  30% {
    opacity: 1;
    transform: translateY(-2px);
  }
}
@media (prefers-reduced-motion: reduce) {
  .ag-dots i {
    animation: none;
    opacity: 0.6;
  }
}
.ag-thinking {
  animation: ag-breathe var(--anim-loop) infinite;
}
@keyframes ag-breathe {
  0%,
  100% {
    opacity: 0.45;
  }
  50% {
    opacity: 1;
  }
}

/* 历史消息气泡入场；流式输出容器（.ag-msg-stream）不加动画，
   否则每帧更新都会重放入场动画造成闪烁 —— 只让新增消息有入场感 */
.ag-msg:not(.ag-msg-stream) {
  animation: ag-msg-in var(--dur-fast) var(--anim-ease-out);
}
@keyframes ag-msg-in {
  from {
    opacity: 0;
    transform: translateY(3px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
