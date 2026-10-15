import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { SavedActionWire as WireAction } from "../types";
import { useConsoleStore } from "./console";

/**
 * 保存的动作按钮（B5.2）
 *
 * 需求原文：「AI 对话这里需要支持保存任务为功能按钮，比如启动、解读等，
 * 保存为按钮后，再显示到代码区域的头部，然后点击这些按钮的时候就不用再去询问 AI 了，
 * 直接执行对应的操作命令就行了」。
 *
 * 与既有 `stores/recipe.ts` 的关系（重要，别混淆）：
 *  - 配方（recipe）：按 action key 固定的沉淀，带指纹校验（构建文件变了就失效），
 *    面向「构建/测试」这类可复现命令，且要走危险命令硬拦截。
 *  - 保存动作（saved action）：用户随手保存的**任意**命令，可以是「解读」这种非命令动作，
 *    也可以是「启动某服务」这种长驻命令。面向日常操作。
 * 二者互补，不互相替代。
 *
 * 存储位置用 `<项目根>/.easyide/actions.json`（AGENTS.md 规定工作区私有状态放 `.easyide/`），
 * 按钮要跟着项目走，换机器拉下仓库也有 —— 不用 localStorage。
 */

/** 动作类型：执行命令 / 仅提问 AI（不落盘、不执行） */
export type SavedKind = "command" | "prompt";

export interface SavedAction {
  /** 稳定唯一 id（重启后不变，按钮 key 依赖它） */
  id: string;
  /** 展示名（用户可改） */
  label: string;
  kind: SavedKind;
  /** kind=command 时：要执行的命令行 */
  command?: string;
  /** kind=prompt 时：直接交给 AI 的提示词（免去再问一遍「怎么启动」） */
  prompt?: string;
  /** 长驻进程：点击后不等待退出，需要显式停止 */
  background: boolean;
  /** 就绪检测正则；命中则状态从「启动中」升级为「已就绪」 */
  readyPattern?: string;
  /** 图标名（Lucide 路径 key） */
  icon: string;
  /** 归属动作 key（run/start/build…），用于状态关联；空表示独立按钮 */
  actionKey?: string;
  /** 用户排序 */
  order: number;
}

const DEFAULT_ICON = "hammer";

/** 默认图标：按动作语义挑一个贴切的内联图标（AGENTS.md 要求内联 Lucide SVG） */
function defaultIcon(a: { actionKey?: string; background?: boolean }): string {
  if (a.background) return "play";
  switch (a.actionKey) {
    case "run":
    case "start":
      return "play";
    case "build":
      return "archive";
    case "debug":
      return "bug";
    case "test":
      return "check";
    case "deps":
      return "package";
    case "explain":
      return "code";
    case "fix":
      return "wrench";
    default:
      return DEFAULT_ICON;
  }
}

/** 后端返回的是 snake_case，这里统一转成前端模型的 camelCase */
function fromWire(a: WireAction): SavedAction {
  return {
    id: a.id,
    label: a.label,
    kind: a.kind === "prompt" ? "prompt" : "command",
    command: a.command ?? undefined,
    prompt: a.prompt ?? undefined,
    background: !!a.background,
    readyPattern: a.ready_pattern ?? undefined,
    icon: a.icon || DEFAULT_ICON,
    actionKey: a.action_key ?? undefined,
    order: typeof a.order === "number" ? a.order : 0,
  };
}

function toWire(a: SavedAction): WireAction {
  return {
    id: a.id,
    label: a.label,
    kind: a.kind,
    command: a.command ?? null,
    prompt: a.prompt ?? null,
    background: a.background,
    ready_pattern: a.readyPattern ?? null,
    icon: a.icon,
    action_key: a.actionKey ?? null,
    order: a.order,
  };
}

export const useSavedActionsStore = defineStore("savedActions", {
  state: () => ({
    actions: [] as SavedAction[],
    loaded: false,
  }),
  getters: {
    /** 按 order 排序的按钮列表（RunBar 直接消费） */
    ordered: (s) => [...s.actions].sort((a, b) => a.order - b.order),
    /** 启动类按钮（长驻进程），RunBar 用来优先显示 */
    launchers: (s) =>
      [...s.actions].sort((a, b) => a.order - b.order).filter((a) => a.background),
    has: (s) => (id: string) => s.actions.some((a) => a.id === id),
  },
  actions: {
    /** 打开工作区后加载一次；文件不存在即空列表 */
    async load() {
      try {
        const list = await api.savedActionsList();
        this.actions = (list ?? []).map(fromWire);
      } catch {
        // 文件损坏不该让整个面板打不开，退回空列表
        this.actions = [];
      }
      this.loaded = true;
    },
    async persist() {
      try {
        await api.savedActionsSave(this.actions.map(toWire));
      } catch {
        /* 写失败不阻断 UI：按钮仍在内存里可用 */
      }
    },
    /**
     * 保存一个动作按钮。
     * @param label 展示名（默认取命令首词）
     * @param kind command=直接执行；prompt=交给 AI
     */
    async add(init: {
      label?: string;
      kind: SavedKind;
      command?: string;
      prompt?: string;
      background?: boolean;
      readyPattern?: string;
      actionKey?: string;
      icon?: string;
    }): Promise<SavedAction> {
      const seed = init.command ?? init.prompt ?? "";
      const label = init.label ?? (seed.slice(0, 18) || "action");
      const a: SavedAction = {
        // id 用时间戳 + 随机段：够唯一且无需依赖 crypto.randomUUID（旧 WebView 兼容）
        id: `sa_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
        label,
        kind: init.kind,
        command: init.command,
        prompt: init.prompt,
        background: init.background ?? false,
        readyPattern: init.readyPattern,
        icon: init.icon ?? defaultIcon(init),
        actionKey: init.actionKey,
        order: this.actions.length,
      };
      this.actions.push(a);
      await this.persist();
      return a;
    },
    async update(id: string, patch: Partial<SavedAction>) {
      const a = this.actions.find((x) => x.id === id);
      if (!a) return;
      Object.assign(a, patch);
      await this.persist();
    },
    async remove(id: string) {
      this.actions = this.actions.filter((a) => a.id !== id);
      await this.persist();
    },
    /** 上移/下移调整顺序 */
    async move(id: string, dir: -1 | 1) {
      const list = [...this.actions].sort((a, b) => a.order - b.order);
      const i = list.findIndex((a) => a.id === id);
      const j = i + dir;
      if (i < 0 || j < 0 || j >= list.length) return;
      [list[i], list[j]] = [list[j], list[i]];
      list.forEach((a, idx) => (a.order = idx));
      this.actions = list;
      await this.persist();
    },

    /**
     * 执行一个保存的按钮（B5.2 核心：点一下直接跑，不再询问 AI）。
     *
     * - kind=command → 后端直接 spawn（仍过危险命令拦截），不经过 AI
     * - kind=prompt  → 交给 AI（这是「解读」这类动作，本质就是要问 AI）
     */
    async execute(a: SavedAction): Promise<{ ok: boolean; message: string }> {
      if (a.kind === "prompt" || !a.command) {
        if (!a.prompt) return { ok: false, message: "该按钮没有可执行内容" };
        const { useAgentStore } = await import("./agent");
        const agent = useAgentStore();
        agent.openPanel();
        void agent.run(a.prompt);
        return { ok: true, message: "已交给 AI" };
      }
      try {
        const consoleStore = useConsoleStore();
        const r = await api.savedActionRun(a.command, a.background);
        // 登记到控制台并带上按钮信息，RunBar 才能显示「变绿 + 停止」
        consoleStore.trackSpawned(r.id, a.command, {
          action: a.actionKey,
          background: a.background,
          readyPattern: a.readyPattern,
          pid: r.pid,
          label: a.label,
        });
        // B6：长驻的 Java 服务交给热部署编排，改代码会自动编译并重启
        if (a.background) {
          const { useHotReloadStore } = await import("./hotReload");
          useHotReloadStore().watch(r.id, null, a.command);
        }
        return { ok: true, message: `已启动 PID ${r.pid}` };
      } catch (e) {
        return { ok: false, message: String(e) };
      }
    },
  },
});