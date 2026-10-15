<template>
  <div class="file-explorer" @contextmenu.prevent="showMenu($event, null)">
    <div class="fe-tree">
      <FileTree dir="" @menu="showMenu" @open="openNoteFile" />
    </div>

    <!-- 右键菜单（通用下拉组件） -->
    <DropdownMenu
      :open="menu.visible"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      @select="onMenuSelect"
      @close="menu.visible = false"
    />

    <InputDialog
      v-if="dialog.mode === 'file'"
      :title="tf('fe.newFileIn', { dir: dialog.parent || t('fe.rootDir') })"
      @confirm="confirmNewFile"
      @cancel="dialog.mode = 'none'"
    />
    <InputDialog
      v-if="dialog.mode === 'folder'"
      :title="tf('fe.newFolderIn', { dir: dialog.parent || t('fe.rootDir') })"
      @confirm="confirmNewFolder"
      @cancel="dialog.mode = 'none'"
    />
    <InputDialog
      v-if="dialog.mode === 'rename'"
      :title="t('fe.rename')"
      :initial="dialog.entry?.name"
      @confirm="confirmRename"
      @cancel="dialog.mode = 'none'"
    />
    <InputDialog
      v-if="dialog.mode === 'build'"
      :title="t('bcmd.customPrompt')"
      @confirm="confirmBuildCustom"
      @cancel="dialog.mode = 'none'"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, onUnmounted, reactive } from "vue";
import Icon from "../common/Icon.vue";
import InputDialog from "../common/InputDialog.vue";
import DropdownMenu, { type DropItem } from "../common/DropdownMenu.vue";
import FileTree from "./FileTree.vue";
import { useVaultStore } from "../../stores/vault";
import { useEditorStore } from "../../stores/editor";
import { useNotesIndexStore } from "../../stores/notesIndex";
import { useUiStore } from "../../stores/ui";
import { useConsoleStore } from "../../stores/console";
import { api } from "../../ipc/tauri";
import { t, tf } from "../../i18n";
import type { BuildTarget, FsEntry } from "../../types";

const vault = useVaultStore();
const editor = useEditorStore();
const indexStore = useNotesIndexStore();
const ui = useUiStore();
const consoleStore = useConsoleStore();

const multiSelected = ref(new Set<string>());

const menu = reactive({
  visible: false,
  x: 0,
  y: 0,
  entry: null as FsEntry | null,
  /** 钻取层级：root 常规菜单 → targets 选构建模块 → goals 选目标 */
  mode: "root" as "root" | "targets" | "goals",
  /** 右键处探测到的构建模块（沿目录链向上） */
  builds: [] as BuildTarget[],
  target: null as BuildTarget | null,
});

const dialog = reactive({
  mode: "none" as "none" | "file" | "folder" | "rename" | "build",
  parent: null as string | null,
  entry: null as FsEntry | null,
});

/** 构建目标预置表：label 走 i18n，hint 是可直接粘回终端的命令预览 */
function goalsOf(b: BuildTarget): { args: string; label: string; hint: string }[] {
  const pm = b.pm === "pnpm" ? "pnpm" : b.pm === "yarn" ? "yarn" : "npm";
  const prefix = b.dir
    ? pm === "yarn"
      ? `yarn --cwd ${b.dir}`
      : pm === "pnpm"
        ? `pnpm --dir ${b.dir}`
        : `npm --prefix ${b.dir}`
    : pm;
  if (b.kind === "maven") {
    const f = `mvn -f ${b.file}`;
    return [
      { args: "compile", label: t("bcmd.compile"), hint: `${f} compile` },
      { args: "test", label: t("bcmd.test"), hint: `${f} test` },
      { args: "package", label: t("bcmd.package"), hint: `${f} package` },
      { args: "install -DskipTests", label: t("bcmd.installSkip"), hint: `${f} install -DskipTests` },
      { args: "install", label: t("bcmd.install"), hint: `${f} install` },
      { args: "clean", label: t("bcmd.clean"), hint: `${f} clean` },
      { args: "dependency:tree", label: t("bcmd.depTree"), hint: `${f} dependency:tree` },
    ];
  }
  if (b.kind === "gradle") {
    const g = b.dir ? `gradle -p ${b.dir}` : "gradle";
    return [
      { args: "build", label: t("bcmd.build"), hint: `${g} build` },
      { args: "test", label: t("bcmd.test"), hint: `${g} test` },
      { args: "assemble", label: t("bcmd.assemble"), hint: `${g} assemble` },
      { args: "run", label: t("bcmd.run"), hint: `${g} run` },
      { args: "clean", label: t("bcmd.clean"), hint: `${g} clean` },
      { args: "dependencies", label: t("bcmd.deps"), hint: `${g} dependencies` },
    ];
  }
  // npm：按 package.json scripts 动态生成
  const scripts = b.scripts.length ? b.scripts : ["build", "test"];
  return scripts.map((s) => ({ args: s, label: s, hint: `${prefix} run ${s}` }));
}

/** 菜单项根据右键目标与钻取层级动态生成（V3：代码树不再出现"新建笔记"，笔记归笔记模块） */
const menuItems = computed(() => {
  if (menu.mode === "targets") {
    const items: DropItem[] = menu.builds.map((b, i) => ({
      key: `build-target:${i}`,
      label: `${buildKindLabel(b)} · ${b.name || t("bcmd.rootName")}`,
      icon: b.kind === "npm" ? "zap" : "package",
      hint: b.kind === "npm" ? b.pm : b.kind,
    }));
    items.push({ key: "sep-b", label: "", separator: true });
    items.push({ key: "build-back", label: t("bcmd.back"), icon: "chevrons-left" });
    return items;
  }
  if (menu.mode === "goals" && menu.target) {
    const b = menu.target;
    const items: DropItem[] = goalsOf(b).map((g) => ({
      key: `build-goal:${g.args}`,
      label: g.label,
      icon: "play",
      hint: g.hint,
    }));
    items.push({ key: "sep-c", label: "", separator: true });
    items.push({ key: "build-custom", label: t("bcmd.custom"), icon: "terminal" });
    items.push({ key: "sep-b", label: "", separator: true });
    items.push({ key: "build-back", label: t("bcmd.back"), icon: "chevrons-left" });
    return items;
  }
  const e = menu.entry;
  const newItems: DropItem[] = [
    { key: "new-file", label: t("fe.newFile"), icon: "file-plus" },
    { key: "new-folder", label: t("fe.newFolder"), icon: "folder-plus" },
  ];
  const buildItems: DropItem[] = menu.builds.length
    ? [
        { key: "sep-b", label: "", separator: true },
        {
          key: "build-menu",
          label: t("bcmd.menu"),
          icon: "hammer",
          hint: [...new Set(menu.builds.map(buildKindLabel))].join(" · "),
        },
      ]
    : [];
  if (!e) {
    return [
      ...newItems,
      { key: "sep-i", label: "", separator: true },
      { key: "import", label: t("fe.import"), icon: "download" },
      ...buildItems,
    ];
  }
  if (e.is_dir) {
    return [
      ...newItems,
      { key: "sep-i", label: "", separator: true },
      { key: "import", label: t("fe.import"), icon: "download" },
      ...buildItems,
      { key: "sep", label: "", separator: true },
      { key: "delete", label: t("fe.delete"), icon: "trash-2", danger: true },
    ];
  }
  return [
    { key: "rename", label: t("fe.rename"), icon: "pencil" },
    { key: "duplicate", label: t("fe.duplicate"), icon: "copy" },
    ...buildItems,
    { key: "sep", label: "", separator: true },
    { key: "delete", label: t("fe.delete"), icon: "trash-2", danger: true },
  ] as DropItem[];
});

function buildKindLabel(b: BuildTarget): string {
  return b.kind === "maven" ? "Maven" : b.kind === "gradle" ? "Gradle" : b.pm || "npm";
}

const closeMenu = () => (menu.visible = false);
onMounted(() => {
  window.addEventListener("pointerdown", closeMenu);
  window.addEventListener("emd-multi-select-changed", ((e: CustomEvent<Set<string>>) => {
    multiSelected.value = e.detail;
  }) as EventListener);
});
onUnmounted(() => window.removeEventListener("pointerdown", closeMenu));

async function showMenu(ev: MouseEvent, entry: FsEntry | null) {
  menu.x = ev.clientX;
  menu.y = ev.clientY;
  menu.entry = entry;
  menu.mode = "root";
  menu.target = null;
  menu.builds = [];
  menu.visible = true;
  // 探测右键处的构建体系（纯文件检查，毫秒级）；失败不挡常规菜单
  try {
    menu.builds = await api.buildDetect(entry ? entry.path : null);
  } catch {
    menu.builds = [];
  }
}

async function onMenuSelect(key: string) {
  const e = menu.entry;
  // 构建命令钻取：targets/goals 层不关菜单，仅换层
  if (key === "build-menu") {
    menu.mode = "targets";
    return;
  }
  if (key.startsWith("build-target:")) {
    const i = Number(key.slice("build-target:".length));
    menu.target = menu.builds[i] ?? null;
    if (menu.target) menu.mode = "goals";
    return;
  }
  if (key === "build-back") {
    menu.mode = menu.mode === "goals" ? "targets" : "root";
    return;
  }
  if (key.startsWith("build-goal:")) {
    menu.visible = false;
    void runBuildGoal(key.slice("build-goal:".length), false);
    return;
  }
  if (key === "build-custom") {
    menu.visible = false;
    dialog.mode = "build";
    return;
  }
  menu.visible = false;
  switch (key) {
    case "new-file":
      dialog.mode = "file";
      dialog.parent = e?.is_dir ? e.path : parentOf(e);
      break;
    case "new-note":
      ui.openNewNote(e?.is_dir ? e.path : parentOf(e));
      break;
    case "new-canvas":
      vault.newCanvas(e?.is_dir ? e.path : parentOf(e));
      break;
    case "new-folder":
      openFolderDialog(e?.is_dir ? e.path : parentOf(e));
      break;
    case "import":
      importHere(e?.is_dir ? e.path : parentOf(e));
      break;
    case "duplicate":
      if (e) {
        try { await api.duplicateFile(e.path); await vault.refreshParents(e.path); await indexStore.rebuild(); } catch (err) { alert(String(err)); }
      }
      break;
    case "batchDelete":
      if (multiSelected.value.size > 0) {
        if (!confirm(`Delete ${multiSelected.value.size} files?`)) return;
        try {
          await api.deleteFiles(Array.from(multiSelected.value));
          for (const p of multiSelected.value) await vault.refreshParents(p);
          multiSelected.value = new Set();
          await indexStore.rebuild();
        } catch (err) { alert(String(err)); }
      }
      break;
    case "rename":
      dialog.mode = "rename";
      dialog.entry = e;
      break;
    case "delete":
      deleteEntry(e);
      break;
  }
}

function openFolderDialog(parent: string | null) {
  dialog.mode = "folder";
  dialog.parent = parent;
}

/** 导入外部 md/图片等文件到当前库根目录 */
async function importHere(folder?: string | null) {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const files = await open({
    multiple: true,
    filters: [{ name: t("fe.importFilter"), extensions: ["md", "markdown", "canvas", "png", "jpg", "jpeg", "gif", "webp", "svg", "pdf"] }],
  });
  if (!files || (Array.isArray(files) && files.length === 0)) return;
  const paths = Array.isArray(files) ? files : [files];
  try {
    const imported = await api.importFiles(paths, folder ?? null);
    for (const rel of imported) await vault.refreshParents(rel);
    await vault.refreshDir("");
    await indexStore.rebuild();
  } catch (e) {
    alert(`${t("fe.importFail")}: ${e}`);
  }
}

async function openNoteFile(path: string) {
  await editor.openNote(path);
}

function parentOf(entry: FsEntry | null): string | null {
  if (!entry) return null;
  const parts = entry.path.split("/");
  parts.pop();
  return parts.length ? parts.join("/") : null;
}

async function confirmNewFolder(name: string) {
  dialog.mode = "none";
  await api.createFolder(dialog.parent, name);
  await vault.refreshDir(dialog.parent || "");
}

/** 新建任意文件（代码/配置/文本），空内容起步 */
async function confirmNewFile(name: string) {
  dialog.mode = "none";
  const clean = name.trim().replace(/^\/+|\/+$/g, "");
  if (!clean) return;
  const rel = dialog.parent ? `${dialog.parent}/${clean}` : clean;
  try {
    if (await api.pathExists(rel)) {
      alert(`${t("fe.exists")}: ${rel}`);
      return;
    }
    await api.writeTextFile(rel, "");
    await vault.refreshParents(rel);
    await editor.openNote(rel);
    ui.view = "editor";
  } catch (err) {
    alert(String(err));
  }
}

async function confirmRename(name: string) {
  const entry = dialog.entry;
  dialog.mode = "none";
  if (!entry) return;
  const newPath = await api.renamePath(entry.path, name);
  // 修正编辑器标签与树
  editor.renamePath(entry.path, newPath);
  await vault.refreshParents(newPath);
  await indexStore.rebuild();
}

async function deleteEntry(e: FsEntry | null) {
  if (!e) return;
  if (!confirm(`${tf("fe.confirmDel", { name: e.name })}`)) return;
  await api.deletePath(e.path);
  // 关掉对应标签（若打开）
  editor.removePath(e.path);
  await vault.refreshParents(e.path);
  await indexStore.rebuild();
}

/** 执行构建目标：后端 spawn 后登记控制台（输出页签），与 RunBar 按钮同一套展示 */
async function runBuildGoal(goal: string, custom: boolean) {
  const b = menu.target;
  if (!b) return;
  try {
    const r = await api.buildGoalRun({ kind: b.kind, dir: b.dir, goal, custom });
    consoleStore.trackSpawned(r.id, r.command, {
      action: `build:${b.kind}:${b.dir}:${goal}`,
      label: `${buildKindLabel(b)}${b.name ? " · " + b.name : ""}`,
    });
    ui.consoleTab = "output";
    ui.consoleVisible = true;
  } catch (err) {
    ui.notify(`${t("bcmd.fail")}: ${err}`, "error");
  }
}

/** 自定义目标：如 mvn 的 `clean install -DskipTests`（后端过字符白名单与危险命令拦截） */
async function confirmBuildCustom(goal: string) {
  dialog.mode = "none";
  const g = goal.trim();
  if (!g) return;
  await runBuildGoal(g, true);
}
</script>

<style scoped>
.file-explorer {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.fe-tree {
  flex: 1;
  overflow-y: auto;
  padding: 4px 6px;
}
</style>
