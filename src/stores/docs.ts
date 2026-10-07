import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { DocEntry, DocsScanResult } from "../types";
import { useEditorStore } from "./editor";

export type DocsGroupMode = "dir" | "smart" | "flat" | "date";

/** 项目文档中心状态（知识库为项目维度，状态存 .easyide/md-assistant.json） */
export const useDocsStore = defineStore("docs", {
  state: () => ({
    entries: [] as DocEntry[],
    categories: [] as string[],
    knowledgeDirs: [] as [string, string][],
    aiToolDirs: "",
    showAiToolDocs: false,
    agentsTarget: "AGENTS.md",
    ignoredCount: 0,
    groupMode: "smart" as DocsGroupMode,
    loading: false,
    classify: null as { done: number; total: number } | null,
    /** 最近一次提示信息（zip 路径 / agents 完成 / 错误），面板底部短暂展示 */
    notice: "",
    _scanTimer: null as ReturnType<typeof setTimeout> | null,
  }),
  getters: {
    visibleEntries: (s) =>
      s.showAiToolDocs ? s.entries : s.entries.filter((e) => !e.from_ai_dir),
    /** 未分类（供 AI 批量分类按钮角标） */
    unclassifiedCount: (s) => s.entries.filter((e) => !e.category).length,
  },
  actions: {
    async scan() {
      this.loading = true;
      try {
        const res: DocsScanResult = await api.docsScan();
        this.entries = res.entries;
        this.categories = res.categories;
        this.knowledgeDirs = res.state.knowledge_dirs;
        this.aiToolDirs = res.state.ai_tool_dirs;
        this.showAiToolDocs = res.state.show_ai_tool_docs;
        this.ignoredCount = res.state.ignored_count;
        if (["dir", "smart", "flat", "date"].includes(res.state.default_group)) {
          this.groupMode = res.state.default_group as DocsGroupMode;
        }
      } finally {
        this.loading = false;
      }
    },
    /** 文件变化后的防抖重扫 */
    scanSoon() {
      if (this._scanTimer) clearTimeout(this._scanTimer);
      this._scanTimer = setTimeout(() => this.scan(), 800);
    },
    async setGroupMode(m: DocsGroupMode) {
      this.groupMode = m;
      await api.docsUpdateSettings({ default_group: m });
    },
    async toggleAiDocs(v: boolean) {
      this.showAiToolDocs = v;
      await api.docsUpdateSettings({ show_ai_tool_docs: v });
    },
    async updateKnowledgeDirs(dirs: [string, string][]) {
      this.knowledgeDirs = dirs;
      await api.docsUpdateSettings({ knowledge_dirs: dirs });
    },
    async updateAiDirs(text: string) {
      this.aiToolDirs = text;
      await api.docsUpdateSettings({ ai_tool_dirs: text });
    },
    async ignore(paths: string[]) {
      await api.docsIgnore(paths);
      await this.scan();
    },
    async unignoreAll() {
      await api.docsUnignore(null);
      await this.scan();
    },
    /** 归档移动（物理），同步编辑器标签路径 */
    async move(paths: string[], category: string) {
      const moved = await api.docsMove(paths, category);
      const editor = useEditorStore();
      for (const [oldP, newP] of moved) editor.renamePath(oldP, newP);
      await this.scan();
      return moved.length;
    },
    async zip(items: [string, string][]) {
      const p = await api.docsZip(items);
      this.notice = p;
      return p;
    },
    async generateAgents() {
      const rel = await api.docsGenerateAgents();
      this.notice = rel;
      return rel;
    },
    async aiClassify(paths: string[]) {
      const n = await api.docsAiClassify(paths);
      await this.scan();
      return n;
    },
    async quickOp(path: string, op: string) {
      return await api.docsQuickOp(path, op);
    },
  },
});
