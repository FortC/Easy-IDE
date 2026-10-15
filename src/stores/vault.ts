import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { VcsFileStatus } from "../types";
import type { FsEntry, NoteIndex, OpenVaultResult, VaultEntry } from "../types";
import { useNotesIndexStore } from "./notesIndex";
import { useEditorStore } from "./editor";
import { useUiStore } from "./ui";
import { invalidateLocalSuggest } from "../lib/localSuggest";
import { useHotReloadStore } from "./hotReload";
import { t } from "../i18n";

export const useVaultStore = defineStore("vault", {  state: () => ({
    opened: false,
    root: "",
    vaultList: [] as VaultEntry[],
    /** 目录缓存：rel path -> children（文件树按需加载） */
    dirCache: {} as Record<string, FsEntry[]>,
    /** 已展开的目录 */
    expanded: {} as Record<string, boolean>,
    /** 版本控制工作区状态（path -> M/A/D/R/?），文件树着色用（Git/SVN 统一） */
    vcsFiles: {} as Record<string, string>,
    vcsBranch: "",
    vcsRevision: "",
    /** "git" | "svn" | "none" */
    vcsKind: "none",
    /** 面板手动选择的实现（"git"/"svn"；空 = 自动探测） */
    vcsPreferred: "" as string,
    _vcsTimer: null as ReturnType<typeof setTimeout> | null,
  }),
  actions: {
    async loadVaultList() {
      this.vaultList = await api.listVaults();
    },
    /** 刷新版本控制状态（打开工作区/文件变化防抖调用） */
    async refreshVcs() {
      try {
        const st = await api.vcsStatus(this.vcsPreferred || null);
        const map: Record<string, string> = {};
        for (const f of st.files as VcsFileStatus[]) map[f.path] = f.status;
        this.vcsFiles = map;
        this.vcsBranch = st.branch;
        this.vcsRevision = st.revision;
        this.vcsKind = (st.kind as "git" | "svn" | "none") || "none";
      } catch {
        this.vcsFiles = {};
        this.vcsBranch = "";
        this.vcsRevision = "";
        this.vcsKind = "none";
      }
    },
    /** 面板手动切换实现（git/svn），切不动时回落自动 */
    async switchVcs(kind: string) {
      this.vcsPreferred = kind;
      await this.refreshVcs();
    },
    refreshVcsSoon() {
      if (this._vcsTimer) clearTimeout(this._vcsTimer);
      this._vcsTimer = setTimeout(() => this.refreshVcs(), 600);
    },
    /** 后端已打开 vault（如右键文件打开）时，前端同步状态 */
    applyOpened(root: string, notes: NoteIndex[]) {
      this.root = root;
      this.opened = true;
      useNotesIndexStore().setNotes(notes);
      this.dirCache = {};
      this.expanded = {};
      this.loadDir("");
      this.loadVaultList();
      void useEditorStore().restoreSession();
      // 工作区换了，符号索引也换了 → B7-A 档联想缓存必须失效，否则会联想出上个项目的符号
      invalidateLocalSuggest();
    },
    async open(path: string) {
      const res: OpenVaultResult = await api.openVault(path);
      this.root = res.root;
      this.opened = true;
      const notes = useNotesIndexStore();
      notes.setNotes(res.notes);
      this.dirCache = {};
      this.expanded = {};
      invalidateLocalSuggest();
      // 换工作区同样要解除旧接管（上一个项目的进程 id 在这里已无意义）
      useHotReloadStore().dispose();
      await this.loadDir("");
      await this.loadVaultList();
      void this.refreshVcs();
      await useEditorStore().restoreSession();
    },
    async create(path: string) {
      const res = await api.createVault(path);
      this.root = res.root;
      this.opened = true;
      useNotesIndexStore().setNotes(res.notes as NoteIndex[]);
      this.dirCache = {};
      this.expanded = {};
      invalidateLocalSuggest();
      await this.loadDir("");
      await this.loadVaultList();
    },
    async close() {
      await api.closeVault();
      this.opened = false;
      this.root = "";
      this.dirCache = {};
      this.expanded = {};
      invalidateLocalSuggest();
      // B6：离开工作区必须解除热部署接管，否则旧项目的进程 id 会被带着重启
      useHotReloadStore().dispose();
      useEditorStore().reset();
      useUiStore().reset();
    },
    async forget(path: string) {
      await api.forgetVault(path);
      await this.loadVaultList();
    },
    /** 加载某目录一层（带缓存） */
    async loadDir(rel: string): Promise<FsEntry[]> {
      if (this.dirCache[rel]) return this.dirCache[rel];
      const children = await api.listDir(rel || null);
      this.dirCache = { ...this.dirCache, [rel]: children };
      return children;
    },
    async refreshDir(rel: string) {
      const children = await api.listDir(rel || null);
      this.dirCache = { ...this.dirCache, [rel]: children };
    },
    toggleExpand(rel: string) {
      this.expanded = { ...this.expanded, [rel]: !this.expanded[rel] };
      if (this.expanded[rel]) this.loadDir(rel);
    },
    /** 新建笔记（支持模板内容），并打开 */
    async newNote(folder: string | null, title: string, content = "") {
      const rel = await api.createNote(folder, title, content || "");
      await this.refreshParents(rel);
      await useEditorStore().openNote(rel);
      return rel;
    },
    async refreshParents(rel: string) {
      // 刷新该文件所在目录及祖先链的缓存
      const parts = rel.split("/");
      for (let i = parts.length - 1; i >= 0; i--) {
        const dir = parts.slice(0, i).join("/");
        if (this.dirCache[dir] !== undefined) {
          await this.refreshDir(dir);
        }
      }
    },
    /** 新建画布（.canvas JSON 文件）并打开画布视图 */
    async newCanvas(folder: string | null, name = t("cv.newCanvasName")) {
      const prefix = folder ? `${folder}/` : "";
      let rel = `${prefix}${name}.canvas`;
      let i = 2;
      while (await api.pathExists(rel)) {
        rel = `${prefix}${name} ${i++}.canvas`;
      }
      await api.writeTextFile(rel, '{"nodes":[],"edges":[]}');
      await this.refreshParents(rel);
      const ui = useUiStore();
      ui.canvasPath = rel;
      ui.view = "canvas";
      return rel;
    },
  },
});
