<template>
  <div class="gp-panel">
    <div class="emd-panel-header">
      <Icon name="git-branch" :size="13" />
      <span>{{ branch || t("gt.title") }}</span>
      <span class="gp-count">{{ files.length }}</span>
      <button class="gp-tool-btn" :title="t('gt.refresh')" @click="refresh">
        <Icon name="refresh-cw" :size="12" :class="{ 'is-spin': loading }" />
      </button>
    </div>
    <div class="gp-list">
      <div v-if="!isRepo" class="gp-empty">{{ t("gt.notRepo") }}</div>
      <div v-else-if="files.length === 0" class="gp-empty">{{ t("gt.clean") }}</div>
      <div
        v-for="f in files"
        :key="f.path"
        class="emd-tree-item gp-file"
        :class="{ 'is-active': f.path === editor.activePath }"
        @click="openFile(f)"
        @contextmenu.prevent="showDiff(f)"
      >
        <span class="gp-badge" :class="'s-' + (f.status === '?' ? 'U' : f.status || 'U')">{{ f.status || "?" }}</span>
        <span class="gp-path">{{ f.path }}</span>
        <button class="gp-diff-btn" :title="t('gt.diff')" @click.stop="showDiff(f)">
          <Icon name="file-diff" :size="12" />
        </button>
      </div>
    </div>

    <!-- diff 视图 -->
    <transition name="emd-fade">
      <div v-if="diff.visible" class="emd-modal-bg" @click.self="diff.visible = false">
        <div class="emd-modal gp-diff-modal">
          <div class="gp-diff-head">
            <span class="gp-diff-title">{{ diff.path }}</span>
            <button class="gp-tool-btn" @click="diff.visible = false">
              <Icon name="x" :size="13" />
            </button>
          </div>
          <div class="gp-diff-body">
            <div v-for="(l, i) in diffLines" :key="i" class="gp-diff-line" :class="diffClass(l)">
              <span class="gp-diff-num">{{ l.num }}</span>
              <span class="gp-diff-text">{{ l.text }}</span>
            </div>
            <div v-if="diffLines.length === 0" class="gp-empty">{{ t("gt.noDiff") }}</div>
          </div>
          <div class="gp-diff-actions">
            <button class="emd-btn" @click="copyDiff">
              <Icon name="copy" :size="12" /> {{ t("c.copy") }}
            </button>
            <button class="emd-btn emd-btn-accent" @click="askAiCommit">
              <Icon name="play" :size="12" /> {{ t("gt.aiCommit") }}
            </button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import Icon from "../common/Icon.vue";
import { useEditorStore } from "../../stores/editor";
import { useUiStore } from "../../stores/ui";
import { useAgentStore } from "../../stores/agent";
import { useVaultStore } from "../../stores/vault";
import { api } from "../../ipc/tauri";
import type { GitFileStatus } from "../../types";
import { t } from "../../i18n";

const editor = useEditorStore();
const ui = useUiStore();
const agent = useAgentStore();
const vault = useVaultStore();

const files = ref<GitFileStatus[]>([]);
const loading = ref(false);
const isRepo = ref(true);
const branch = computed(() => vault.gitBranch);

async function refresh() {
  loading.value = true;
  try {
    const st = await api.gitStatus();
    isRepo.value = st.is_repo;
    files.value = st.files;
    await vault.refreshGit();
  } finally {
    loading.value = false;
  }
}

async function openFile(f: GitFileStatus) {
  await editor.openNote(f.path);
  ui.view = "editor";
}

const diff = reactive({ visible: false, path: "", text: "" });

async function showDiff(f: GitFileStatus) {
  diff.path = f.path;
  diff.text = "";
  diff.visible = true;
  diff.text = await api.gitDiff(f.path);
}

interface DiffLine {
  kind: "" | "add" | "del" | "hunk";
  num: string;
  text: string;
}

const diffLines = computed<DiffLine[]>(() =>
  diff.text.split("\n").map((line, i) => {
    const kind: DiffLine["kind"] = line.startsWith("+++") || line.startsWith("---")
      ? ""
      : line.startsWith("+")
        ? "add"
        : line.startsWith("-")
          ? "del"
          : line.startsWith("@@")
            ? "hunk"
            : "";
    return { kind, num: kind ? line.slice(0, 1) : "", text: line };
  }),
);

function diffClass(l: DiffLine): string {
  return l.kind ? `is-${l.kind}` : "";
}

async function copyDiff() {
  await navigator.clipboard.writeText(diff.text);
}

function askAiCommit() {
  diff.visible = false;
  const list = files.value.map((f) => `${f.status} ${f.path}`).join("\n");
  agent.openPanel();
  void agent.run(
    `请根据下面这些 Git 变更文件生成一条符合规范的提交信息（格式：type: 描述，如 fix: / feat: / refactor:），并给出对应的 git add 与 git commit 命令（用 run_command 执行前会请求我确认）。\n\n${list}`,
  );
}

onMounted(refresh);
</script>

<style scoped>
.gp-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.gp-count {
  margin-left: auto;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.gp-tool-btn {
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.gp-tool-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.is-spin {
  animation: gp-spin 1s linear infinite;
}
@keyframes gp-spin {
  to {
    transform: rotate(360deg);
  }
}
.gp-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 6px;
}
.gp-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 24px 8px;
  font-size: var(--font-ui-size);
}
.gp-file {
  gap: 6px;
}
.gp-badge {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  font-size: 10px;
  font-weight: 700;
  background: var(--background-modifier-hover);
  color: var(--text-muted);
}
.gp-badge.s-M {
  color: #e5a03c;
}
.gp-badge.s-A,
.gp-badge.s-U {
  color: #4ec970;
}
.gp-badge.s-D {
  color: var(--text-error, #e93147);
}
.gp-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-ui-size);
}
.gp-file.is-active .gp-path {
  color: var(--text-accent);
}
.gp-diff-btn {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-faint);
  opacity: 0;
  transition: opacity var(--anim-fast);
}
.gp-file:hover .gp-diff-btn {
  opacity: 1;
}
.gp-diff-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}

/* diff 弹窗 */
.gp-diff-modal {
  width: 760px;
  height: 560px;
  padding: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.gp-diff-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.gp-diff-title {
  flex: 1;
  font-weight: 600;
  color: var(--text-normal);
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.gp-diff-body {
  flex: 1;
  overflow: auto;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  line-height: 1.5;
  padding: 6px 0;
}
.gp-diff-line {
  display: flex;
  gap: 8px;
  padding: 0 12px;
}
.gp-diff-line .gp-diff-num {
  flex-shrink: 0;
  width: 10px;
  color: var(--text-faint);
}
.gp-diff-line .gp-diff-text {
  white-space: pre-wrap;
  overflow-wrap: break-word;
  color: var(--text-muted);
}
.gp-diff-line.is-add .gp-diff-text {
  color: #4ec970;
  background: rgba(78, 201, 112, 0.08);
}
.gp-diff-line.is-del .gp-diff-text {
  color: var(--text-error, #e93147);
  background: rgba(233, 49, 71, 0.08);
}
.gp-diff-line.is-hunk .gp-diff-text {
  color: var(--interactive-accent);
}
.gp-diff-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding: 8px 12px;
  border-top: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
</style>
