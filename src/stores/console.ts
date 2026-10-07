import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import { useUiStore } from "./ui";

export interface ConsoleLine {
  line: string;
  stderr: boolean;
}

export interface ConsoleRun {
  id: number;
  command: string;
  startedAt: number;
  status: "running" | "done" | "failed";
  code: number | null;
  lines: ConsoleLine[];
}

/** 控制台：命令运行记录与输出流（事件驱动，全局唯一） */
export const useConsoleStore = defineStore("console", {
  state: () => ({
    runs: [] as ConsoleRun[],
    _resolvers: {} as Record<number, (r: { code: number | null }) => void>,
  }),
  getters: {
    running: (s) => s.runs.filter((r) => r.status === "running"),
    /** 最近一次运行的末尾输出（修复报错场景用） */
    tail(): string {
      const run = this.runs[this.runs.length - 1];
      if (!run) return "";
      return run.lines
        .slice(-40)
        .map((l) => l.line)
        .join("\n");
    },
  },
  actions: {
    /** 事件入口：App.vue 挂载时接线一次 */
    bindEvents() {
      void api.onConsoleOutput((e) => {
        const run = this.runs.find((r) => r.id === e.id);
        if (!run) return;
        run.lines.push({ line: e.line, stderr: e.stderr });
        if (run.lines.length > 2000) run.lines.splice(0, run.lines.length - 2000);
      });
      void api.onConsoleExit((e) => {
        const run = this.runs.find((r) => r.id === e.id);
        if (run) {
          run.status = e.code === 0 || e.code === null ? "done" : "failed";
          run.code = e.code;
        }
        this._resolvers[e.id]?.({ code: e.code });
        delete this._resolvers[e.id];
      });
    },
    /** 执行命令：返回 Promise 在进程退出时结算；控制台自动弹出 */
    async run(command: string): Promise<{ code: number | null; tail: string }> {
      const id = await api.agentRunCommand(command);
      const run: ConsoleRun = {
        id,
        command,
        startedAt: Date.now(),
        status: "running",
        code: null,
        lines: [],
      };
      this.runs.push(run);
      if (this.runs.length > 30) this.runs.splice(0, this.runs.length - 30);
      useUiStore().consoleVisible = true;
      const code = await new Promise<number | null>((resolve) => {
        this._resolvers[id] = (r) => resolve(r.code);
        // 兜底：如果事件已错过（几乎不可能），60s 后查一次状态
        setTimeout(() => {
          if (this._resolvers[id]) {
            const r = this.runs.find((x) => x.id === id);
            if (r && r.status !== "running") {
              resolve(r.code);
              delete this._resolvers[id];
            }
          }
        }, 2000);
      });
      const finished = this.runs.find((x) => x.id === id);
      const tail = (finished?.lines ?? [])
        .slice(-60)
        .map((l) => l.line)
        .join("\n");
      return { code, tail };
    },
    kill(id: number) {
      void api.agentKillCommand(id);
    },
    killAll() {
      for (const r of this.runs) {
        if (r.status === "running") this.kill(r.id);
      }
    },
    clear() {
      this.runs = this.runs.filter((r) => r.status === "running");
    },
  },
});
