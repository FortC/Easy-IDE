<template>
  <div class="gp-panel">
    <div class="emd-panel-header">
      <Icon name="git-branch" :size="13" />
      <span class="gp-kind">{{ kindLabel }}</span>
      <span>{{ headLabel }}</span>
      <span class="gp-count">{{ files.length }}</span>
      <button class="gp-tool-btn" :title="t('gt.refresh')" @click="refresh">
        <Icon name="refresh-cw" :size="12" :class="{ 'is-spin': loading }" />
      </button>
      <button
        class="gp-tool-btn"
        :title="allExpanded ? t('gt.collapseAll') : t('gt.expandAll')"
        @click="toggleAll"
      >
        <Icon :name="allExpanded ? 'minimize-2' : 'maximize-2'" :size="12" />
      </button>
    </div>
    <!-- 实现切换（.git 与 .svn 共存时可手动切换，V3-D3） -->
    <div class="gp-kind-switch">
      <button class="gp-kind-btn" :class="{ 'is-active': currentKind === 'git' }" @click="switchTo('git')">Git</button>
      <button class="gp-kind-btn" :class="{ 'is-active': currentKind === 'svn' }" @click="switchTo('svn')">SVN</button>
    </div>
    <!-- 仓库根在工作区子目录：说明为什么只看到部分文件（B3） -->
    <div v-if="isRepo && repoRoot" class="gp-nested">
      <Icon name="folder" :size="11" />
      <span>{{ tf("gt.nestedRepo", { path: repoRoot }) }}</span>
    </div>
    <div class="gp-list">
      <div v-if="!isRepo" class="gp-empty">{{ t("gt.notRepo") }}</div>
      <div v-else-if="files.length === 0" class="gp-empty">{{ t("gt.clean") }}</div>
      <!-- 树形层级展示：按路径拆分聚合，目录可展开/折叠；避免「每行都是完整路径」的平铺冗余（B4） -->
      <div
        v-for="n in flatNodes"
        :key="n.path"
        class="emd-tree-item gp-node"
        :class="{
          'is-dir': n.isDir,
          'is-active': !n.isDir && n.path === editor.activePath,
        }"
        :style="{ paddingLeft: 6 + n.depth * 14 + 'px' }"
        @click="n.isDir ? toggle(n.path) : openByPath(n.path, n.status)"
        @contextmenu.prevent="!n.isDir && showDiffByPath(n.path)"
      >
        <Icon
          v-if="n.isDir"
          :name="expanded.has(n.path) ? 'chevron-down' : 'chevron-right'"
          :size="12"
          class="gp-chevron"
        />
        <span v-else class="gp-badge" :class="'s-' + badgeClass(n.status)">{{ n.status || "?" }}</span>
        <Icon
          v-if="n.isDir"
          name="folder"
          :size="13"
          :class="['gp-diricon', expanded.has(n.path) ? 'is-open' : '']"
        />
        <span class="gp-name">{{ n.name }}</span>
        <button
          v-if="!n.isDir"
          class="gp-diff-btn"
          :title="t('gt.diff')"
          @click.stop="showDiffByPath(n.path)"
        >
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
import type { VcsFileStatus } from "../../types";
import { t, tf } from "../../i18n";

const editor = useEditorStore();
const ui = useUiStore();
const agent = useAgentStore();
const vault = useVaultStore();

const files = ref<VcsFileStatus[]>([]);
const loading = ref(false);
const isRepo = ref(true);
/** 仓库根相对工作区根的路径（null = 仓库根就是工作区根） */
const repoRoot = ref<string | null>(null);
/** 树形展开状态：记录已展开目录的完整路径；刷新后保留用户折叠/展开习惯 */
const expanded = ref<Set<string>>(new Set());
const currentKind = computed(() => (vault.vcsKind === "none" ? (vault.vcsPreferred || "git") : vault.vcsKind));
const kindLabel = computed(() => (currentKind.value === "svn" ? "SVN" : "Git"));
const headLabel = computed(() => {
  const b = vault.vcsBranch;
  const r = vault.vcsRevision;
  if (b && r && currentKind.value === "svn") return `${b}@${r}`;
  if (b && r) return `${b} · ${r}`;
  return b || r || t("gt.title");
});

/** 树节点：目录与文件同构，用 isDir 区分；status 仅叶子有 */
interface TreeNode {
  name: string;
  path: string;
  isDir: boolean;
  status?: string;
  children: TreeNode[];
}

/** 按路径拆分构建嵌套树：同前缀的变更文件会自然聚合到共同父目录 */
function buildTree(list: VcsFileStatus[]): TreeNode[] {
  const root: TreeNode = { name: "", path: "", isDir: true, children: [] };
  for (const f of list) {
    const parts = f.path.split("/").filter(Boolean);
    if (parts.length === 0) continue;
    let cur = root;
    for (let i = 0; i < parts.length; i++) {
      const isLast = i === parts.length - 1;
      const name = parts[i];
      const path = parts.slice(0, i + 1).join("/");
      let child = cur.children.find((c) => c.name === name);
      if (!child) {
        child = {
          name,
          path,
          isDir: !isLast,
          children: [],
          status: isLast ? f.status : undefined,
        };
        cur.children.push(child);
      }
      cur = child;
    }
  }
  // 目录在前（按名升序），文件在后；递归保证每层都排好
  const sort = (n: TreeNode) => {
    n.children.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    for (const c of n.children) sort(c);
  };
  sort(root);
  return root.children;
}

/** 拍扁为模板可直接渲染的有序列表：按 expanded 决定是否下钻 */
function flatten(nodes: TreeNode[], depth: number, out: FlatNode[]) {
  for (const n of nodes) {
    out.push({
      name: n.name,
      path: n.path,
      depth,
      isDir: n.isDir,
      status: n.status,
    });
    if (n.isDir && expanded.value.has(n.path)) {
      flatten(n.children, depth + 1, out);
    }
  }
}

interface FlatNode {
  name: string;
  path: string;
  depth: number;
  isDir: boolean;
  status?: string;
}

const flatNodes = computed<FlatNode[]>(() => {
  const tree = buildTree(files.value);
  const out: FlatNode[] = [];
  flatten(tree, 0, out);
  return out;
});

/** 首次拿到文件时把顶层目录全部展开，让用户一眼看到全貌；下层保持折叠避免长滚 */
function ensureDefaultExpand() {
  if (expanded.value.size > 0) return;
  const tree = buildTree(files.value);
  for (const n of tree) {
    if (n.isDir) expanded.value.add(n.path);
  }
}

function toggle(p: string) {
  // Set 没有响应式变更检测，复制再赋值触发刷新
  const next = new Set(expanded.value);
  if (next.has(p)) next.delete(p);
  else next.add(p);
  expanded.value = next;
}

/** 收集所有目录节点的 path：用于「全部展开/折叠」一键切换 */
function collectDirPaths(nodes: TreeNode[], out: string[]) {
  for (const n of nodes) {
    if (n.isDir) out.push(n.path);
    if (n.children.length) collectDirPaths(n.children, out);
  }
}

const allExpanded = computed(() => {
  const tree = buildTree(files.value);
  const dirs: string[] = [];
  collectDirPaths(tree, dirs);
  if (dirs.length === 0) return false;
  for (const d of dirs) if (!expanded.value.has(d)) return false;
  return true;
});

function toggleAll() {
  const tree = buildTree(files.value);
  const dirs: string[] = [];
  collectDirPaths(tree, dirs);
  if (allExpanded.value) {
    expanded.value = new Set();
  } else {
    expanded.value = new Set(dirs);
  }
}

function openByPath(p: string, status?: string) {
  return openFile({ path: p, status: (status ?? "?") as VcsFileStatus["status"] });
}
function showDiffByPath(p: string) {
  return showDiff({ path: p, status: "?" as VcsFileStatus["status"] });
}
function badgeClass(status?: string): string {
  if (status === "?") return "U";
  return status || "U";
}

async function refresh() {
  loading.value = true;
  try {
    const st = await api.vcsStatus(vault.vcsPreferred || null);
    isRepo.value = st.is_repo;
    files.value = st.files;
    repoRoot.value = st.repo_root ?? null;
    ensureDefaultExpand();
    await vault.refreshVcs();
  } finally {
    loading.value = false;
  }
}

async function switchTo(kind: "git" | "svn") {
  if (currentKind.value === kind) return;
  await vault.switchVcs(kind);
  await refresh();
}

async function openFile(f: VcsFileStatus) {
  await editor.openNote(f.path);
  ui.view = "editor";
}

const diff = reactive({ visible: false, path: "", text: "" });

async function showDiff(f: VcsFileStatus) {
  diff.path = f.path;
  diff.text = "";
  diff.visible = true;
  diff.text = await api.vcsDiff(f.path, vault.vcsPreferred || null);
}

interface DiffLine {
  kind: "" | "add" | "del" | "hunk";
  num: string;
  text: string;
}

const diffLines = computed<DiffLine[]>(() =>
  diff.text.split("\n").map((line) => {
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
  const vcsName = currentKind.value === "svn" ? "SVN" : "Git";
  agent.openPanel();
  void agent.run(
    `请根据下面这些 ${vcsName} 变更文件生成一条符合规范的提交信息（格式：type: 描述，如 fix: / feat: / refactor:），并给出对应的 add 与 commit 命令（用 run_command 执行前会请求我确认）。\n\n${list}`,
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
.gp-kind {
  color: var(--text-accent);
  font-weight: 600;
  flex-shrink: 0;
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
  animation: gp-spin var(--anim-loop) linear infinite;
}
@keyframes gp-spin {
  to {
    transform: rotate(360deg);
  }
}
.gp-kind-switch {
  display: flex;
  gap: 4px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
/* 仓库根在子目录时的说明条：解释「为什么只看到部分变更」 */
.gp-nested {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  background: var(--background-primary-alt);
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  line-height: 1.4;
  flex-shrink: 0;
}
.gp-nested svg {
  flex-shrink: 0;
  opacity: 0.7;
}
.gp-kind-btn {
  padding: 3px 10px;
  border-radius: var(--radius-s);
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
}
.gp-kind-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.gp-kind-btn.is-active {
  background: var(--background-modifier-active-hover);
  color: var(--text-normal);
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
.gp-node {
  gap: 5px;
}
.gp-chevron {
  flex-shrink: 0;
  color: var(--text-faint);
  transition: transform var(--anim-hover);
}
.gp-diricon {
  flex-shrink: 0;
  color: var(--text-muted);
  transition: color var(--anim-hover);
}
.gp-diricon.is-open {
  color: var(--text-accent);
}
.gp-node.is-dir {
  color: var(--text-normal);
  font-weight: 600;
}
.gp-node.is-dir:hover {
  background: var(--background-modifier-hover);
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
  color: var(--text-error);
}
.gp-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--font-ui-size);
}
.gp-node.is-active .gp-name {
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
.gp-node:hover .gp-diff-btn {
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
  color: var(--text-error);
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
