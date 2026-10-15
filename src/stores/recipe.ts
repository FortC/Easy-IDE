import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { CommandLogEntry, RecipeInput, RecipeStatus } from "../types";
import { useConsoleStore } from "./console";
import { useProjectStore } from "./project";
import { useSettingsStore } from "./settings";

/** 配方提案（配方卡待确认内容） */
export interface RecipeProposal {
  action: string;
  /** AI 执行过的命令候选（含退出码） */
  candidates: CommandLogEntry[];
  pick: number;
  command: string;
  cwd: string;
  readyPattern: string;
  fingerprintFiles: string[];
  conversationId: string;
  prompt: string;
}

/** 快速通道轻确认（未信任配方执行前） */
export interface RecipeConfirm {
  action: string;
  command: string;
  resolve: (v: "cancel" | "once" | "trust") => void;
}

/** 命令配方中心（V2 核心）：捕获 → 沉淀 → 快速通道复用（零 AI） */
export const useRecipeStore = defineStore("recipe", {
  state: () => ({
    statuses: [] as RecipeStatus[],
    /** 弹出的配方卡（捕获沉淀） */
    proposal: null as RecipeProposal | null,
    /** 弹出的轻确认（未信任直通前） */
    pendingConfirm: null as RecipeConfirm | null,
  }),
  getters: {
    /** 动作的配方状态（取首个同动作配方） */
    statusOf:
      (s) =>
      (action: string): RecipeStatus | null =>
        s.statuses.find((r) => r.action === action && !r.variant) ??
        s.statuses.find((r) => r.action === action) ??
        null,
  },
  actions: {
    async refresh() {
      try {
        this.statuses = await api.recipeCheck();
      } catch {
        this.statuses = [];
      }
    },

    /** 从 AI 命令记录生成配方提案（弹配方卡，用户确认后沉淀） */
    propose(
      action: string,
      log: CommandLogEntry[],
      meta: { conversationId: string; prompt: string },
    ) {
      if (!log.length) return;
      // run 取最后一条（长驻进程可能被用户手动停止，退出码不代表失败）；其余取最后成功
      const succeeded = log.filter((c) => c.code === 0);
      const candidates = action === "run" ? log : succeeded.length ? succeeded : log;
      const pick = candidates.length - 1;
      const project = useProjectStore();
      this.proposal = {
        action,
        candidates,
        pick,
        command: candidates[pick].command,
        cwd: "",
        readyPattern: "",
        fingerprintFiles: project.fingerprintFiles,
        conversationId: meta.conversationId,
        prompt: meta.prompt,
      };
    },
    setPick(i: number) {
      const p = this.proposal;
      if (!p || i < 0 || i >= p.candidates.length) return;
      p.pick = i;
      p.command = p.candidates[i].command;
    },
    async confirmProposal() {
      const p = this.proposal;
      if (!p) return;
      const input: RecipeInput = {
        action: p.action,
        command: p.command.trim(),
        cwd: p.cwd.trim(),
        ready_pattern: p.readyPattern.trim(),
        source: "ai",
        model: useSettingsStore().data.ai_model,
        conversation_id: p.conversationId,
        prompt: p.prompt,
      };
      await api.recipeSave(input);
      this.proposal = null;
      await this.refresh();
    },
    rejectProposal() {
      this.proposal = null;
    },

    /** 快速通道执行（零 AI）；返回 true = 已处理（含用户取消），false = 回落 AI 推理 */
    async runFast(action: string): Promise<boolean> {
      const st = this.statusOf(action);
      if (!st || !st.valid) return false;
      if (!st.trusted) {
        const verdict = await new Promise<"cancel" | "once" | "trust">((resolve) => {
          this.pendingConfirm = { action, command: st.command, resolve };
        });
        this.pendingConfirm = null;
        if (verdict === "cancel") return true; // 用户主动取消，不回落 AI
        if (verdict === "trust") await api.recipeTrust(action, st.variant, true);
      }
      const res = await api.recipeRun(action, st.variant);
      if (!res.ok) {
        await this.refresh();
        return false; // 失效/缺失 → 回落 AI
      }
      const consoleStore = useConsoleStore();
      const started = Date.now();
      if (res.cmd_id != null) {
        void consoleStore.attach(res.cmd_id, res.command).then(({ code }) => {
          void api.recipeRecordUsage(action, st.variant, code, Date.now() - started);
          void this.refresh();
        });
      }
      return true;
    },

    resolveConfirm(v: "cancel" | "once" | "trust") {
      this.pendingConfirm?.resolve(v);
    },
  },
});
