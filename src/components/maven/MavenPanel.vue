<template>
  <div class="mv-panel">
    <div class="emd-panel-header">
      <Icon name="package" :size="13" />
      <span>{{ t("mv.title") }}</span>
      <span v-if="conflictCount > 0" class="mv-conflict-badge">{{ conflictCount }}</span>
      <span class="mv-count">{{ modules.length ? modules.length + " mod" : "" }}</span>
      <button class="mv-tool-btn" :title="t('mv.refresh')" :disabled="loading" @click="load">
        <Icon name="refresh-cw" :size="12" :class="{ 'is-spin': loading }" />
      </button>
    </div>

    <div class="mv-filter">
      <input v-model="filter" type="text" class="mv-filter-input" :placeholder="t('mv.filterPh')" />
      <label class="mv-only-conflict">
        <input v-model="onlyConflict" type="checkbox" />
        {{ t("mv.onlyConflict") }}
      </label>
    </div>

    <div class="mv-tree">
      <div v-if="error" class="mv-error">{{ error }}</div>
      <div v-else-if="loading" class="mv-empty">{{ t("mv.loading") }}</div>
      <div v-else-if="flatNodes.length === 0" class="mv-empty">
        {{ modules.length === 0 ? t("mv.empty") : t("mv.noMatch") }}
      </div>
      <template v-else>
        <div v-for="node in flatNodes" :key="node.node.gav + node.idx" class="mv-row-wrap">
          <div
            class="emd-tree-item mv-row"
            :style="{ paddingLeft: 6 + node.node.depth * 12 + 'px' }"
            :title="node.node.gav"
            @contextmenu.prevent="showMenu($event, node)"
          >
            <span class="mv-art">{{ node.node.artifact }}</span>
            <span class="mv-ver" :class="{ 'is-conflict': node.node.conflict_with }">
              {{ node.node.version }}
            </span>
            <span v-if="node.node.scope" class="mv-scope">{{ node.node.scope }}</span>
            <span v-if="node.node.conflict_with" class="mv-conflict" :title="t('mv.conflictTip')">
              ⚠ {{ node.node.conflict_with }}
            </span>
          </div>
        </div>
      </template>
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
import { computed, onMounted, reactive, ref } from "vue";
import Icon from "../common/Icon.vue";
import DropdownMenu, { type DropItem } from "../common/DropdownMenu.vue";
import { api } from "../../ipc/tauri";
import type { DepNode, ModuleTree } from "../../types";
import { useAgentStore } from "../../stores/agent";
import { t, tf } from "../../i18n";

const agent = useAgentStore();

const modules = ref<ModuleTree[]>([]);
const loading = ref(false);
const error = ref("");
const filter = ref("");
const onlyConflict = ref(false);

interface FlatNode {
  node: DepNode;
  /** 根→此节点的父链（不含根），用于定位"挂在哪个直接依赖下"以生成排除 */
  parents: DepNode[];
  idx: number;
}

const flatNodes = computed<FlatNode[]>(() => {
  const out: FlatNode[] = [];
  const q = filter.value.trim().toLowerCase();
  let idx = 0;
  for (const m of modules.value) {
    walk(m.root, []);
  }
  function walk(node: DepNode, parents: DepNode[]) {
    const hit =
      (!q || node.gav.toLowerCase().includes(q)) &&
      (!onlyConflict.value || node.conflict_with);
    if (hit && node.depth > 0) {
      out.push({ node, parents: [...parents], idx: idx++ });
    }
    if (q || onlyConflict.value) {
      // 过滤态：仍递归找匹配项（父子展示可能断开，可接受）
    }
    const next = [...parents, node];
    for (const c of node.children) walk(c, node.depth === 0 ? [] : next);
  }
  return out.slice(0, 500);
});

const conflictCount = computed(
  () => countConflicts(modules.value.map((m) => m.root)),
);

function countConflicts(nodes: DepNode[]): number {
  let n = 0;
  const walkAll = (node: DepNode) => {
    if (node.conflict_with) n++;
    node.children.forEach(walkAll);
  };
  nodes.forEach(walkAll);
  return n;
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    modules.value = await api.mavenDepTree();
  } catch (e) {
    error.value = String(e);
    modules.value = [];
  } finally {
    loading.value = false;
  }
}

// ---- 右键操作 ----
const menu = reactive({ visible: false, x: 0, y: 0, target: null as FlatNode | null });

function showMenu(ev: MouseEvent, node: FlatNode) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.target = node;
  menu.visible = true;
}

const menuItems = computed<DropItem[]>(() => [
  { key: "copy", label: t("mv.copyGav") },
  { key: "exclude", label: t("mv.exclude"), icon: "x" },
  { key: "ai", label: t("mv.aiAnalyze"), icon: "play" },
]);

async function onMenuSelect(key: string) {
  menu.visible = false;
  const target = menu.target;
  if (!target) return;
  if (key === "copy") {
    void navigator.clipboard.writeText(target.node.gav);
  } else if (key === "exclude") {
    await applyExclusion(target);
  } else if (key === "ai") {
    agent.openPanel();
    void agent.run(
      `Maven 依赖出现版本冲突，请分析并给出处理建议。冲突依赖：${target.node.gav} 被版本 ${target.node.conflict_with} 挤出；它的引入链：${chainText(target)}。可以用 read_file 查看 pom.xml 后，给出应该在哪依赖上加什么 exclusion，并说明理由。`,
    );
  }
}

function chainText(target: FlatNode): string {
  const names = [...target.parents.filter((p) => p.depth > 0).map((p) => p.gav), target.node.gav];
  return names.join(" → ");
}

/** 一键排除：exclusion 加到引入链上的第一个直接依赖（深度 1）里 */
async function applyExclusion(target: FlatNode) {
  const direct = [...target.parents].reverse().find((p) => p.depth === 1);
  if (!direct) {
    alert(t("mv.noDirectDep"));
    return;
  }
  const excl =
    target.parents.length > 1
      ? target.parents[target.parents.length - 1] // 排除"直接拉入冲突版本的上一级"
      : target.node;
  const ok = confirm(
    tf("mv.confirmExclude", {
      dep: `${direct.group}:${direct.artifact}`,
      excl: `${excl.group}:${excl.artifact}`,
    }),
  );
  if (!ok) return;
  try {
    await api.mavenApplyExclusion(direct.group, direct.artifact, excl.group, excl.artifact);
    alert(t("mv.excludeDone"));
  } catch (e) {
    alert(String(e));
  }
}

onMounted(load);
</script>

<style scoped>
.mv-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.mv-count {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.mv-conflict-badge {
  background: rgba(233, 49, 71, 0.14);
  color: var(--text-error, #e93147);
  border-radius: var(--radius-s);
  padding: 0 5px;
  font-size: 10px;
  font-weight: 700;
}
.mv-tool-btn {
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.mv-tool-btn:hover:not(:disabled) {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.mv-tool-btn:disabled {
  opacity: 0.5;
}
.is-spin {
  animation: mv-spin 1s linear infinite;
}
@keyframes mv-spin {
  to {
    transform: rotate(360deg);
  }
}
.mv-filter {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.mv-filter-input {
  flex: 1;
  min-width: 0;
  height: 24px;
  font-size: var(--font-ui-size);
}
.mv-only-conflict {
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  white-space: nowrap;
  cursor: pointer;
}
.mv-tree {
  flex: 1;
  overflow-y: auto;
  padding: 4px 6px;
}
.mv-empty,
.mv-error {
  color: var(--text-faint);
  text-align: center;
  padding: 16px 10px;
  font-size: var(--font-ui-size);
  white-space: pre-wrap;
}
.mv-error {
  color: var(--text-error, #e93147);
}
.mv-row {
  gap: 6px;
}
.mv-art {
  font-size: var(--font-ui-size);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mv-ver {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  flex-shrink: 0;
}
.mv-ver.is-conflict {
  color: var(--text-error, #e93147);
  text-decoration: line-through;
}
.mv-scope {
  color: var(--text-faint);
  font-size: 10px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
  padding: 0 4px;
  flex-shrink: 0;
}
.mv-conflict {
  color: #e5a03c;
  font-size: var(--font-ui-smaller);
  flex-shrink: 0;
}
</style>
