import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { ProjectModel } from "../types";

/** 项目模型：打开工作区探测一次（类型/模块/任务槽/指纹文件） */
export const useProjectStore = defineStore("project", {
  state: () => ({
    model: null as ProjectModel | null,
    loaded: false,
  }),
  getters: {
    /** 动作 → 探测建议命令（无则空串） */
    suggestOf: (s) => (action: string) => s.model?.suggests[action] ?? "",
    fingerprintFiles: (s) => s.model?.fingerprint_files ?? [],
  },
  actions: {
    async refresh() {
      try {
        this.model = await api.projectDetect();
      } catch {
        this.model = null;
      }
      this.loaded = true;
    },
  },
});
