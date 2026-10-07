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
          <!-- 工具调用块 -->
          <div v-if="it.tool" class="ag-tool" :class="{ 'is-rejected': it.tool.rejected }">
            <button class="ag-tool-head" @click="toggleTool(i)">
              <Icon name="terminal" :size="12" />
              <span class="ag-tool-name">{{ it.tool.name }}</span>
              <span class="ag-tool-args">{{ toolSummary(it.tool) }}</span>
              <Icon :name="openTools.has(i) ? 'chevron-down' : 'chevron-right'" :size="12" />
            </button>
            <div v-if="openTools.has(i)" class="ag-tool-body">
              <pre class="ag-pre">{{ it.tool.result }}</pre>
            </div>
          </div>
          <div
            v-if="stripToolJson(it.content)"
            class="ag-bubble ag-bubble-ai md-body"
            v-html="render(stripToolJson(it.content))"
          />
        </div>
      </template>

      <!-- 流式输出中 -->
      <div v-if="agent.streaming" class="ag-msg ag-msg-ai">
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
        <div v-else-if="agent.streamText" class="ag-bubble ag-bubble-ai md-body" v-html="render(agent.streamText)" />
        <!-- 占位：pendingCommand 分支在上方，保持链完整 -->
        <div v-else class="ag-thinking">
          <Icon name="refresh-cw" :size="13" class="is-spin" /> {{ t("ag.thinking") }}
        </div>
      </div>

      <div v-if="agent.error" class="ag-error">{{ agent.error }}</div>
    </div>

    <!-- 输入区 -->
    <div class="ag-input-area">
      <textarea
        v-model="draft"
        class="ag-input"
        :placeholder="t('ag.inputPh')"
        :disabled="agent.streaming"
        rows="3"
        @keydown.ctrl.enter.prevent="send"
        @keydown.meta.enter.prevent="send"
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
import { nextTick, onMounted, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import DiffView from "../common/DiffView.vue";
import { useAgentStore } from "../../stores/agent";
import { renderMarkdown } from "../../lib/markdown/renderer";
import { t } from "../../i18n";

const agent = useAgentStore();
const draft = ref("");
const listEl = ref<HTMLElement>();
const openTools = ref(new Set<number>());

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

function render(md: string): string {
  return renderMarkdown(md, { resolveNote: () => null, attachmentsDir: "assets" });
}

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
.ag-thinking {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-faint);
  font-size: var(--font-ui-size);
  padding: 4px 2px;
}
.is-spin {
  animation: ag-spin 1s linear infinite;
}
@keyframes ag-spin {
  to {
    transform: rotate(360deg);
  }
}
.ag-error {
  color: var(--text-error, #e93147);
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
</style>
