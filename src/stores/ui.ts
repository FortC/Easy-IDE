import { defineStore } from "pinia";
import type { LeftTab, MainView, RightTab, WorkContext } from "../types";
import { isMarkdown } from "../lib/filetypes";
import { useEditorStore } from "./editor";
import { t } from "../i18n";

/** 右栏 tab 的显示名（随语境切换，见 rightTabs getter） */
const RIGHT_TAB_LABELS: Record<RightTab, () => string> = {
  ai: () => t("ws.ai"),
  outline: () => t("ws.outline"),
  backlinks: () => t("ws.backlinks"),
  props: () => t("ws.props"),
  refs: () => t("ws.refs"),
  memory: () => t("ws.memory"),
};

/** 左栏属于笔记语境的面板 */
const NOTE_PANELS: LeftTab[] = ["notes", "jots", "tags", "docs"];

export const useUiStore = defineStore("ui", {
  state: () => ({
    view: "editor" as MainView,
    leftVisible: true,
    leftTab: "code" as LeftTab,
    rightVisible: true,
    /** 变更后由 rightTabs getter 保证其属于当前语境的集合 */
    rightTab: "ai" as RightTab,
    searchOpen: false,
    searchMode: "file" as "file" | "content" | "class" | "symbol",
    settingsOpen: false,
    /** 新建笔记对话框 */
    newNoteOpen: false,
    newNoteFolder: null as string | null,
    /** 底部控制台 */
    consoleVisible: false,
    /** 控制台标签：输出 | 终端 | 问题（问题页签仅代码语境） */
    consoleTab: "output" as "output" | "terminal" | "problems",
    /** 增量包对话框 */
    pkgOpen: false,
    /** 当前打开的 canvas 文件（相对路径） */
    canvasPath: "",
    /** 当前打开的图谱："" = 全库引用图谱（自动），否则 .graph 相对路径 */
    graphPath: "",
    /** 图片预览（V3：图片不再走文本编辑链路） */
    imagePreview: "",
    /** 全局瞬时提示（B2：按钮执行失败等需要反馈的场景） */
    toast: { msg: "", kind: "info" as "info" | "error" | "success", seq: 0 },
  }),
  getters: {
    /**
     * 语境判定（双语境产品的核心抽象）
     * 由「左栏面板 + 当前打开文件类型」双因子决定：
     * - 左栏在笔记类面板 → note
     * - 打开 .md 文件 → note
     * 其余（代码面板 / 打开代码文件）→ dev
     */
    context(state): WorkContext {
      if (NOTE_PANELS.includes(state.leftTab)) return "note";
      const editor = useEditorStore();
      if (editor.activePath && isMarkdown(editor.activePath)) return "note";
      return "dev";
    },

    /**
     * 右栏 tab 集随语境自适应
     * - dev：AI / 代码大纲 / 引用查找（符号级）/ AI 记忆
     * - note：AI / 大纲 / 反链（md 双向链接）/ 属性（frontmatter）
     * 图谱 / 日历是左侧大模块（活动栏入口 → 主区全幅视图），不占右栏 tab。
     * 两种语境结构对称（AI · 大纲 · 链接类 · 元信息类），切换语境肌肉记忆一致
     */
    rightTabs(): { key: RightTab; label: string }[] {
      const tabs: RightTab[] =
        this.context === "dev"
          ? ["ai", "outline", "refs", "memory"]
          : ["ai", "outline", "backlinks", "props"];
      return tabs.map((key) => ({ key, label: RIGHT_TAB_LABELS[key]() }));
    },

    /** 开发语境模块（代码/版本控制/Maven） */
    devModule(state): boolean {
      return state.leftTab === "code" || state.leftTab === "vcs" || state.leftTab === "maven";
    },
    /** RunBar 仅代码语境常驻（笔记语境的动作已并入 AI 快捷 chips） */
    showRunBar(): boolean {
      return this.context === "dev";
    },
    /** 底部「问题」页签仅代码语境有意义 */
    showProblemTab(): boolean {
      return this.context === "dev";
    },
  },
  actions: {
    /** 弹一条瞬时提示；seq 递增用于重复消息也能触发 Transition 重新播放 */
    /** kind=success 走 --text-success（与 RunBar 的「已就绪」绿一致），info 中性，error 报错 */
    notify(msg: string, kind: "info" | "error" | "success" = "info") {
      this.toast = { msg, kind, seq: this.toast.seq + 1 };
      const seq = this.toast.seq;
      if (typeof window !== "undefined") {
        window.setTimeout(() => {
          // 只清自己那条，避免连续提示互相盖掉
          if (this.toast.seq === seq) this.toast.msg = "";
        }, 2600);
      }
    },
    reset() {
      this.view = "editor";
      this.leftTab = "code";
      this.rightTab = "ai";
      this.searchOpen = false;
      this.settingsOpen = false;
      this.newNoteOpen = false;
      this.canvasPath = "";
      this.graphPath = "";
      this.imagePreview = "";
    },
    openSearch(mode: "file" | "content" | "class" | "symbol") {
      this.searchMode = mode;
      this.searchOpen = true;
    },
    openNewNote(folder: string | null = null) {
      this.newNoteFolder = folder;
      this.newNoteOpen = true;
    },
    /** 切换右栏 tab；不在当前语境集合内则回落 AI（保证「AI 永不为空」） */
    setRightTab(tab: RightTab) {
      this.rightTab = tab;
    },
    /**
     * 语境变化后校正当前 tab：不在新集合内则回落 AI。
     * 不重置 AI 会话，只切换上下文来源。
     */
    reconcileRightTab() {
      const valid = this.rightTabs.map((tab) => tab.key);
      if (!valid.includes(this.rightTab)) {
        this.rightTab = "ai";
      }
    },
  },
});
