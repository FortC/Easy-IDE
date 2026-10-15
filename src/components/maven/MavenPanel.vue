<template>
  <div class="mv-panel">
    <div class="emd-panel-header">
      <Icon name="package" :size="13" />
      <span>{{ t("mv.title") }}</span>
      <span class="mv-count">
        {{ view === "mods" ? modTree.length + " " + t("mv.modCount") : modules.length ? modules.length + " mod" : "" }}
      </span>
    </div>

    <div class="mv-viewtabs" role="tablist">
      <button
        class="mv-viewtab"
        :class="{ 'is-active': view === 'deps' }"
        role="tab"
        :aria-selected="view === 'deps'"
        @click="view = 'deps'"
      >
        {{ t("mv.viewDeps") }}
        <span v-if="conflictCount > 0" class="mv-conflict-badge">{{ conflictCount }}</span>
      </button>
      <button
        class="mv-viewtab"
        :class="{ 'is-active': view === 'mods' }"
        role="tab"
        :aria-selected="view === 'mods'"
        @click="switchToModules"
      >
        {{ t("mv.viewModules") }}
        <span v-if="modTree.length" class="mv-tab-count">{{ modTree.length }}</span>
      </button>
      <button
        class="mv-tool-btn"
        :title="t('mv.refresh')"
        :disabled="loading || modLoading"
        @click="loadCurrent"
      >
        <Icon name="refresh-cw" :size="12" :class="{ 'is-spin': loading || modLoading }" />
      </button>
    </div>

    <!-- 依赖树视图（mvn dependency:tree） -->
    <template v-if="view === 'deps'">
      <div class="mv-filter">
        <select v-if="projectModules.length > 0" v-model="selectedModule" class="mv-module-select" @change="load">
          <option value="">{{ t("mv.rootModule") }}</option>
          <option v-for="m in projectModules" :key="m.path" :value="m.path">{{ m.name }}</option>
        </select>
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
    </template>

    <!-- 模块结构视图（B4 离线解析 pom.xml，不依赖 mvn） -->
    <template v-else>
      <div class="mv-modlist">
        <div v-if="modLoading" class="mv-empty">{{ t("mv.modLoading") }}</div>
        <div v-else-if="modTree.length === 0" class="mv-empty">{{ t("mv.modEmpty") }}</div>
        <template v-else>
          <div
            v-for="m in modTree"
            :key="m.rel_path"
            class="mv-modrow"
            :style="{ paddingLeft: 6 + m.depth * 14 + 'px' }"
            :title="m.rel_path + ' · ' + m.group_id"
            @click="viewModuleDeps(m)"
            @contextmenu.prevent="showModMenu($event, m)"
          >
            <Icon name="package" :size="12" class="mv-mod-icon" />
            <span class="mv-mod-art">{{ m.artifact_id || m.rel_path }}</span>
            <span class="mv-mod-ver">{{ m.version || "—" }}</span>
            <span v-if="m.packaging" class="mv-scope">{{ m.packaging }}</span>
            <span v-if="m.is_boot_app" class="mv-mod-boot" :title="t('mv.modBoot')">
              {{ t("mv.modBoot") }}
            </span>
            <span v-if="m.dep_count" class="mv-mod-meta">{{ m.dep_count }}</span>
            <span v-if="m.error" class="mv-mod-err" :title="m.error">{{ t("mv.modParseErr") }}</span>
          </div>
        </template>
        <div class="mv-moddeps" v-if="modDeps.length">
          <div class="mv-moddeps-h">{{ modDepsTitle }}</div>
          <div v-for="(d, i) in modDeps" :key="i" class="mv-moddep">
            <span class="mv-moddep-ga">{{ d.group_id }}:{{ d.artifact_id }}</span>
            <span class="mv-moddep-v">{{ d.version || "—" }}</span>
            <span v-if="d.scope && d.scope !== 'compile'" class="mv-scope">{{ d.scope }}</span>
          </div>
        </div>
      </div>
    </template>

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
import type { DepNode, ModuleTree, PomDep, PomModuleNode, ProjectModule } from "../../types";
import { useAgentStore } from "../../stores/agent";
import { useUiStore } from "../../stores/ui";
import { t, tf } from "../../i18n";

const agent = useAgentStore();
const ui = useUiStore();

const modules = ref<ModuleTree[]>([]);
const loading = ref(false);
const error = ref("");
const filter = ref("");
const onlyConflict = ref(false);
/** 多模块选择（V3-P1：maven_dep_tree 支持模块参数） */
const projectModules = ref<ProjectModule[]>([]);
const selectedModule = ref("");

// ---- B4 双视图 ----
type View = "deps" | "mods";
const view = ref<View>("deps");
/** 离线模块树（B4：沿 <modules> 递归解析，不依赖 mvn，秒开） */
const modTree = ref<PomModuleNode[]>([]);
const modLoading = ref(false);
/** 点击某个模块后展开的离线依赖明细 */
const modDeps = ref<PomDep[]>([]);
const modDepsTitle = ref("");

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
    modules.value = await api.mavenDepTree(selectedModule.value || null);
  } catch (e) {
    error.value = String(e);
    modules.value = [];
  } finally {
    loading.value = false;
  }
}

/** 刷新按钮按当前视图刷对应数据（离线树很轻，但显式刷新语义更清楚） */
function loadCurrent() {
  return view.value === "mods" ? loadModTree() : load();
}

// ---- B4 模块结构（离线） ----

/** 首次切到模块视图时才拉取，之后缓存；离线解析很轻，但也不必每次点击都跑 */
async function switchToModules() {
  view.value = "mods";
  if (modTree.value.length) return;
  await loadModTree();
}

async function loadModTree() {
  modLoading.value = true;
  try {
    modTree.value = await api.mavenModuleTree();
  } catch (e) {
    // 离线通道失败不应让面板变白板：降级为空 + 提示
    modTree.value = [];
    modLoading.value = false;
    ui.notify(String(e), "error");
    return;
  }
  modLoading.value = false;
}

async function viewModuleDeps(m: PomModuleNode) {
  modDepsTitle.value = `${m.artifact_id || m.rel_path} (${m.rel_path})`;
  try {
    modDeps.value = await api.mavenModuleDeps(m.rel_path === "." ? null : m.rel_path);
  } catch (e) {
    modDeps.value = [];
    ui.notify(String(e), "error");
  }
}

// ---- 右键操作 ----
const menu = reactive({
  visible: false,
  x: 0,
  y: 0,
  target: null as FlatNode | null,
  modTarget: null as PomModuleNode | null,
});

function showMenu(ev: MouseEvent, node: FlatNode) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.target = node;
  menu.modTarget = null;
  menu.visible = true;
}

function showModMenu(ev: MouseEvent, m: PomModuleNode) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.target = null;
  menu.modTarget = m;
  menu.visible = true;
}

const menuItems = computed<DropItem[]>(() => {
  if (menu.modTarget) {
    const items: DropItem[] = [{ key: "mod-deps", label: t("mv.modDepsOf"), icon: "package" }];
    if (menu.modTarget.is_boot_app) {
      items.push({ key: "mod-run", label: t("mv.modBootRun"), icon: "play" });
    }
    return items;
  }
  return [
    { key: "copy", label: t("mv.copyGav") },
    { key: "exclude", label: t("mv.exclude"), icon: "x" },
    { key: "ai", label: t("mv.aiAnalyze"), icon: "play" },
  ];
});

async function onMenuSelect(key: string) {
  menu.visible = false;
  const mod = menu.modTarget;
  if (mod) {
    if (key === "mod-deps") {
      await viewModuleDeps(mod);
    } else if (key === "mod-run") {
      // 交给 AI 决定命令形态（不同项目可能是 spring-boot:run / java -jar），这里只给足上下文
      agent.openPanel();
      void agent.run(
        `请启动 Maven 模块 ${mod.rel_path}（artifactId=${mod.artifact_id}）。它含 spring-boot-maven-plugin，请用 mvn spring-boot:run 在该模块目录下启动，并把进程放在后台运行、不要等待退出。`,
      );
    }
    return;
  }
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
    await api.mavenApplyExclusion(direct.group, direct.artifact, excl.group, excl.artifact, selectedModule.value || null);
    alert(t("mv.excludeDone"));
  } catch (e) {
    alert(String(e));
  }
}

onMounted(async () => {
  try {
    const model = await api.projectDetect();
    // project_model 会把含 spring-boot-maven-plugin 的模块细分为 "maven-spring-boot"，
    // 它仍是 Maven 模块，用 startsWith 才能都收进来（旧写法 startsWith("maven") 亦可，此处显式化）
    projectModules.value = (model.modules || []).filter(
      (m) => !m.kind || m.kind === "maven" || m.kind === "maven-spring-boot",
    );
  } catch {
    projectModules.value = [];
  }
  await load();
});
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
  color: var(--text-error);
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
  animation: mv-spin var(--anim-loop) linear infinite;
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
.mv-module-select {
  height: 24px;
  max-width: 120px;
  font-size: var(--font-ui-smaller);
  color: var(--text-normal);
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
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

/* ---- B4 双视图 ---- */
.mv-viewtabs {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 4px 6px 0;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
}
.mv-viewtab {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 24px;
  padding: 0 9px;
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  border-radius: var(--radius-s) var(--radius-s) 0 0;
  position: relative;
}
.mv-viewtab:hover:not(.is-active) {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.mv-viewtab.is-active {
  color: var(--text-normal);
  font-weight: 600;
  background: var(--background-secondary);
}
/* 选中下划线用 inset 阴影，避免撑高元素导致 tab 行跳动 */
.mv-viewtab.is-active::after {
  content: "";
  position: absolute;
  left: 6px;
  right: 6px;
  bottom: -1px;
  height: 2px;
  border-radius: 1px;
  background: var(--accent, var(--text-accent));
}
.mv-viewtabs .mv-tool-btn {
  margin-left: auto;
}
.mv-tab-count {
  color: var(--text-faint);
  font-size: 10px;
}

.mv-modlist {
  flex: 1;
  overflow-y: auto;
  padding: 4px 6px;
}
.mv-modrow {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 24px;
  border-radius: var(--radius-s);
  font-size: var(--font-ui-size);
  cursor: pointer;
}
.mv-modrow:hover {
  background: var(--background-modifier-hover);
}
.mv-mod-icon {
  color: var(--text-faint);
  flex-shrink: 0;
}
.mv-mod-art {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mv-mod-ver {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  flex-shrink: 0;
}
.mv-mod-boot {
  color: var(--text-success);
  border: 1px solid currentColor;
  border-radius: var(--radius-s);
  padding: 0 4px;
  font-size: 10px;
  flex-shrink: 0;
}
.mv-mod-meta {
  margin-left: auto;
  color: var(--text-faint);
  font-size: 10px;
  flex-shrink: 0;
}
.mv-mod-err {
  color: var(--text-error);
  font-size: 10px;
  flex-shrink: 0;
}
.mv-moddeps {
  margin-top: 8px;
  padding-top: 6px;
  border-top: 1px solid var(--background-modifier-border);
}
.mv-moddeps-h {
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
  margin-bottom: 4px;
}
.mv-moddep {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 21px;
  font-size: var(--font-ui-smaller);
}
.mv-moddep-ga {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mv-moddep-v {
  margin-left: auto;
  color: var(--text-faint);
  flex-shrink: 0;
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
  color: var(--text-error);
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
  color: var(--text-error);
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
