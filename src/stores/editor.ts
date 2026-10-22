import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { EditMode } from "../types";
import { useVaultStore } from "./vault";

/** 每个标签的文档状态 */
interface DocState {
  content: string;
  /** 已保存到磁盘的内容（判断脏状态） */
  savedContent: string;
  /** 临时打开时的原始文件绝对路径；保存时除写临时副本外，还回写该文件 */
  originAbs?: string | null;
}

/** 会话恢复上限：防止历史项目一次拉起过多标签 */
const MAX_SESSION_TABS = 12;

/** 当前打开的标签集合与编辑模式（多标签） */
export const useEditorStore = defineStore("editor", {
  state: () => ({
    /** 打开的标签（相对路径，有序） */
    tabs: [] as string[],
    /** 每个标签的文档内容 */
    docs: {} as Record<string, DocState>,
    activePath: "" as string,
    mode: "split" as EditMode,
    saving: false,
    /** 每次切换活动文档递增，供编辑器组件感知"切换文件" */
    openToken: 0,
    /** 切换后跳转目标：定位到某行/锚点 */
    pendingJump: null as { line?: number; anchor?: string } | null,
    _saveTimer: null as ReturnType<typeof setTimeout> | null,
  }),
  getters: {
    isOpen: (s) => s.activePath !== "",
    doc: (s) => s.docs[s.activePath] ?? null,
    content(): string {
      return this.doc?.content ?? "";
    },
    savedContent(): string {
      return this.doc?.savedContent ?? "";
    },
    isDirty(): boolean {
      const d = this.doc;
      return !!d && d.content !== d.savedContent;
    },
    /** 某个标签是否为脏（标签栏圆点） */
    tabDirty: (s) => (path: string) => {
      const d = s.docs[path];
      return !!d && d.content !== d.savedContent;
    },
    wordCount: (s) => {
      const text = (s.docs[s.activePath]?.content ?? "")
        .replace(/[#*`~\[\]()!>-]/g, " ")
        .replace(/\s+/g, " ")
        .trim();
      if (!text) return 0;
      // 中英混排：CJK 字符按字计数，其它按词
      const cjk = (text.match(/[\u4e00-\u9fff\u3400-\u4dbf]/g) || []).length;
      const words = text
        .replace(/[\u4e00-\u9fff\u3400-\u4dbf]/g, " ")
        .split(/\s+/)
        .filter(Boolean).length;
      return cjk + words;
    },
  },
  actions: {
    /** 打开文件：已有标签则激活，否则新增标签；originAbs 为临时打开时的原始文件路径 */
    async openNote(
      path: string,
      jump: { line?: number; anchor?: string } | null = null,
      originAbs: string | null = null,
    ) {
      if (!this.docs[path]) {
        const content = await api.readTextFile(path);
        this.docs = { ...this.docs, [path]: { content, savedContent: content, originAbs } };
        this.tabs = [...this.tabs, path];
      } else if (originAbs && this.docs[path].originAbs !== originAbs) {
        // 同名副本换了来源（再次临时打开别的同名文件），更新映射
        this.docs[path].originAbs = originAbs;
      }
      this.activePath = path;
      this.pendingJump = jump;
      this.openToken++;
      this.persistSession();
    },
    /** 激活已有标签 */
    activateTab(path: string) {
      if (!this.docs[path] || path === this.activePath) return;
      this.activePath = path;
      this.pendingJump = null;
      this.openToken++;
      this.persistSession();
    },
    /** 拖拽排序：把 from 位置的标签移动到 to 位置 */
    moveTab(from: number, to: number) {
      if (from === to || from < 0 || from >= this.tabs.length) return;
      const next = [...this.tabs];
      const [moved] = next.splice(from, 1);
      next.splice(Math.max(0, Math.min(next.length, to)), 0, moved);
      this.tabs = next;
      this.persistSession();
    },
    /** 关闭标签（脏内容先保存，临时文件同样回写原文件） */
    async closeTab(path: string) {
      const d = this.docs[path];
      if (d && d.content !== d.savedContent) {
        try {
          await this.persistDoc(path, d);
        } catch {
          /* 保存失败也要允许关标签，尽力保存后放弃 */
        }
      }
      const idx = this.tabs.indexOf(path);
      this.tabs = this.tabs.filter((p) => p !== path);
      const next = { ...this.docs };
      delete next[path];
      this.docs = next;
      if (this.activePath === path) {
        this.activePath = this.tabs[Math.min(idx, this.tabs.length - 1)] ?? "";
        this.openToken++;
      }
      this.persistSession();
    },
    /** 关闭除指定标签外的全部 */
    async closeOthers(keep: string) {
      for (const p of [...this.tabs]) {
        if (p !== keep) await this.closeTab(p);
      }
    },
    /** 关闭指定标签右侧的全部 */
    async closeRight(path: string) {
      const idx = this.tabs.indexOf(path);
      if (idx < 0) return;
      for (const p of [...this.tabs].slice(idx + 1)) {
        await this.closeTab(p);
      }
    },
    /** 关闭全部 */
    async closeAll() {
      for (const p of [...this.tabs]) {
        await this.closeTab(p);
      }
    },
    /** Ctrl+Tab 循环切换 */
    cycleTab(dir: 1 | -1) {
      const n = this.tabs.length;
      if (n < 2) return;
      const idx = this.tabs.indexOf(this.activePath);
      const next = this.tabs[(idx + dir + n) % n];
      this.activateTab(next);
    },
    setContent(c: string) {
      const d = this.doc;
      if (!d) return;
      d.content = c;
      this.scheduleSave();
    },
    scheduleSave() {
      if (this._saveTimer) clearTimeout(this._saveTimer);
      this._saveTimer = setTimeout(() => this.save(), 400);
    },
    /** 统一落盘：先写工作区内文件；临时打开的文档再回写原始文件 */
    async persistDoc(path: string, d: DocState) {
      await api.writeTextFile(path, d.content);
      if (d.originAbs) {
        const ok = await api.saveTempBack(path, d.content);
        // 客户端记录了来源但映射已丢（如换设备恢复会话），必须当作失败而非静默丢稿
        if (!ok) throw new Error("临时文件的原始来源记录丢失，无法回写");
      }
    },
    async save() {
      const d = this.doc;
      if (!this.activePath || !d || d.content === d.savedContent) return;
      this.saving = true;
      try {
        await this.persistDoc(this.activePath, d);
        d.savedContent = d.content;
      } catch (e) {
        // 自动保存失败必须可见，否则用户以为存上了（脏点还在，但要给出原因）
        const { useUiStore } = await import("./ui");
        useUiStore().notify(`保存失败：${String(e)}`, "error");
      } finally {
        this.saving = false;
      }
    },
    cycleMode() {
      const order: EditMode[] = ["source", "split", "preview"];
      const next = order[(order.indexOf(this.mode) + 1) % order.length];
      this.setMode(next);
    },
    setMode(m: EditMode) {
      this.mode = m;
      this.persistSession();
    },
    reset() {
      if (this._saveTimer) clearTimeout(this._saveTimer);
      this.tabs = [];
      this.docs = {};
      this.activePath = "";
      this.pendingJump = null;
      this.openToken++;
    },
    /** 文件被外部（重命名）后同步路径：标签与文档映射一起搬 */
    renameSelf(newPath: string) {
      if (!this.activePath) return;
      const old = this.activePath;
      const d = this.docs[old];
      if (!d) return;
      const docs = { ...this.docs };
      delete docs[old];
      docs[newPath] = d;
      this.docs = docs;
      this.tabs = this.tabs.map((p) => (p === old ? newPath : p));
      this.activePath = newPath;
      this.persistSession();
    },
    /** 同步任一路径的重命名（不一定是活动文档） */
    renamePath(oldPath: string, newPath: string) {
      if (oldPath === this.activePath) {
        this.renameSelf(newPath);
        return;
      }
      const d = this.docs[oldPath];
      if (!d) return;
      const docs = { ...this.docs };
      delete docs[oldPath];
      docs[newPath] = d;
      this.docs = docs;
      this.tabs = this.tabs.map((p) => (p === oldPath ? newPath : p));
      this.persistSession();
    },
    /** 标签被外部（删除文件）后移除 */
    removePath(path: string) {
      if (this.docs[path]) void this.closeTab(path);
    },
    /** 外部写入（Agent 提案落盘等）后同步标签内容，不触发自动保存 */
    reloadDoc(path: string, content: string) {
      const d = this.docs[path];
      if (!d) return;
      d.content = content;
      d.savedContent = content;
      if (this.activePath === path) {
        this.pendingJump = null;
        this.openToken++;
      }
    },
    // ---- 会话持久化（P0 用 localStorage 按 workspace 隔离；P2 迁往 .easyide/） ----
    sessionKey(): string {
      const root = useVaultStore().root;
      return root ? `easyide.session:${root}` : "";
    },
    persistSession() {
      const key = this.sessionKey();
      if (!key) return;
      try {
        // origins：临时打开文档的原始文件路径，重启恢复后保存仍能回写
        const origins: Record<string, string> = {};
        for (const p of this.tabs) {
          const o = this.docs[p]?.originAbs;
          if (o) origins[p] = o;
        }
        localStorage.setItem(
          key,
          JSON.stringify({ tabs: this.tabs, active: this.activePath, mode: this.mode, origins }),
        );
      } catch {
        /* 配额满等异常不致命 */
      }
    },
    /** 打开工作区后恢复上次会话（文件已不存在则跳过） */
    async restoreSession() {
      const key = this.sessionKey();
      if (!key) return;
      let saved: {
        tabs?: string[];
        active?: string;
        mode?: EditMode;
        origins?: Record<string, string>;
      } | null = null;
      try {
        saved = JSON.parse(localStorage.getItem(key) || "null");
      } catch {
        return;
      }
      if (!saved || !Array.isArray(saved.tabs) || saved.tabs.length === 0) return;
      const valid: string[] = [];
      for (const p of saved.tabs.slice(0, MAX_SESSION_TABS)) {
        if (this.docs[p]) {
          valid.push(p);
          continue;
        }
        try {
          const content = await api.readTextFile(p);
          this.docs = {
            ...this.docs,
            [p]: { content, savedContent: content, originAbs: saved.origins?.[p] ?? null },
          };
          valid.push(p);
        } catch {
          /* 文件已被删除/移走：跳过该标签 */
        }
      }
      this.tabs = valid;
      const active = valid.includes(saved.active || "") ? saved.active! : valid[valid.length - 1];
      if (active) {
        this.activePath = active;
        this.pendingJump = null;
        this.openToken++;
      }
      if (saved.mode) this.mode = saved.mode;
    },
  },
});
