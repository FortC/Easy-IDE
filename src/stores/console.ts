import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import { useUiStore } from "./ui";

export interface ConsoleLine {
  line: string;
  stderr: boolean;
}

/**
 * 运行状态五态（B1/B2）
 *
 * ready 是关键：长驻进程（Spring Boot / vite dev）spawn 成功后进程还活着，
 * 但要等输出命中 readyPattern（如 `Started .* on port`）才算「就绪」。
 * 这样 UI 才能区分「正在启动」和「已启动可访问」。
 */
export type RunStatus = "running" | "ready" | "done" | "failed" | "killed";

export interface ConsoleRun {
  id: number;
  command: string;
  startedAt: number;
  status: RunStatus;
  code: number | null;
  lines: ConsoleLine[];
  /** 操作系统进程 id（B1：启动后可立即显示，不必等退出） */
  pid: number;
  /** 关联的动作 key（run / start / build…），供 B2 查「这个动作在不在跑」 */
  action?: string;
  /** 触发它的按钮名（B2 在 UI 上显示「启动」而不是命令行） */
  label?: string;
  /** 长驻进程标记：需要显式停止，不会自己退出 */
  background: boolean;
  /** 就绪检测正则；命中则 running → ready */
  readyPattern?: string;
  /** 就绪时刻 */
  readyAt?: number;
  /** 用户是否主动停止过（用于把「我杀的」与「自己崩的」区分开） */
  stoppedByUser?: boolean;
}

export interface SpawnOutcome {
  id: number;
  pid: number;
}

/** 单次输出扫描的步数上限：避免每行都全量重扫正则（长输出时是性能坑） */
const READY_SCAN_WINDOW = 400;

function isReadyRun(run: ConsoleRun): boolean {
  if (run.status !== "running" || !run.readyPattern) return false;
  let re: RegExp;
  try {
    re = new RegExp(run.readyPattern, "i");
  } catch {
    // 非法正则不该让整条命令崩掉，视为无就绪检测
    return false;
  }
  const from = Math.max(0, run.lines.length - READY_SCAN_WINDOW);
  return run.lines.slice(from).some((l) => !l.stderr && re.test(l.line));
}

/** 控制台：命令运行记录与输出流（事件驱动，全局唯一） */
export const useConsoleStore = defineStore("console", {
  state: () => ({
    runs: [] as ConsoleRun[],
    _resolvers: {} as Record<number, (r: { code: number | null }) => void>,
  }),
  getters: {
    /** 仍在跑（含已就绪的长驻进程） */
    running: (s) => s.runs.filter((r) => r.status === "running" || r.status === "ready"),
    /** 仅「还在启动中」：用于区分 ▶ / ● 两种视觉态 */
    starting: (s) => s.runs.filter((r) => r.status === "running"),
    /** 已就绪的长驻进程 */
    ready: (s) => s.runs.filter((r) => r.status === "ready"),
    /** 最近一次运行的末尾输出（修复报错场景用） */
    tail(): string {
      const run = this.runs[this.runs.length - 1];
      if (!run) return "";
      return run.lines
        .slice(-40)
        .map((l) => l.line)
        .join("\n");
    },
    /** 按动作 key 找「最近一条未结束的运行」—— B2 的 RunBar 状态真源 */
    activeOf(): (action: string) => ConsoleRun | undefined {
      return (action: string) => {
        for (let i = this.runs.length - 1; i >= 0; i--) {
          const r = this.runs[i];
          if (r.action === action && (r.status === "running" || r.status === "ready")) return r;
        }
        return undefined;
      };
    },
    /** 按动作 key 找「最近一次运行」（无论结束与否），用于显示上次退出码 */
    lastOf(): (action: string) => ConsoleRun | undefined {
      return (action: string) => {
        for (let i = this.runs.length - 1; i >= 0; i--) {
          if (this.runs[i].action === action) return this.runs[i];
        }
        return undefined;
      };
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
        // 就绪检测：只在尚未 ready 时扫，且只扫末尾窗口
        if (run.status === "running" && isReadyRun(run)) {
          run.status = "ready";
          run.readyAt = Date.now();
        }
      });
      void api.onConsoleExit((e) => {
        const run = this.runs.find((r) => r.id === e.id);
        if (run) {
          // 被用户主动停止的进程退出码非 0，标 killed 以便 UI 区分「我杀的」与「自己崩的」
          if (e.code === 0 || e.code === null) run.status = "done";
          else if (run.stoppedByUser) run.status = "killed";
          else run.status = "failed";
          run.code = e.code;
        }
        this._resolvers[e.id]?.({ code: e.code });
        delete this._resolvers[e.id];
        // B6：进程没了就得解除热部署接管，否则会反复去重启一个不存在的进程
        // 动态 import 避免 hotReload → console → hotReload 的循环引用
        void import("./hotReload").then((m) => m.useHotReloadStore().unwatch(e.id));
      });
    },
    /**
     * 非阻塞启动（B1 核心）：spawn 成功后**立即返回**，不等进程退出。
     *
     * 这就是原来 AI 对话卡死的根因修复点：以前只有 `attach()`（阻塞到退出），
     * 启动长驻进程会把 AI 主循环 await 到天荒地老。
     */
    async spawn(
      command: string,
      opts: { action?: string; background?: boolean; readyPattern?: string } = {},
    ): Promise<SpawnOutcome> {
      const r = await api.agentRunCommand(command);
      this._track(r.id, command, {
        action: opts.action,
        background: opts.background ?? false,
        readyPattern: opts.readyPattern,
      });
      useUiStore().consoleVisible = true;
      return r;
    },
    /** 把已 spawn 的命令纳入跟踪（配方快速通道 / 后台 spawn 复用） */
    _track(
      id: number,
      command: string,
      opts: {
        action?: string;
        background?: boolean;
        readyPattern?: string;
        pid?: number;
        label?: string;
      } = {},
    ) {
      const existing = this.runs.find((r) => r.id === id);
      if (existing) {
        // 已跟踪则补齐元信息（配方快速通道先 spawn 后track 的场景）
        existing.action = opts.action ?? existing.action;
        existing.background = opts.background ?? existing.background;
        existing.readyPattern = opts.readyPattern ?? existing.readyPattern;
        existing.label = opts.label ?? existing.label;
        if (opts.pid) existing.pid = opts.pid;
        return existing;
      }
      const run: ConsoleRun = {
        id,
        pid: opts.pid ?? 0,
        command,
        startedAt: Date.now(),
        status: "running",
        code: null,
        lines: [],
        action: opts.action,
        label: opts.label,
        background: opts.background ?? false,
        readyPattern: opts.readyPattern,
      };
      this.runs.push(run);
      if (this.runs.length > 30) this.runs.splice(0, this.runs.length - 30);
      return run;
    },
    /** 公开入口：把后端已 spawn 的进程登记进来（B5.2 按钮直接执行走这条） */
    trackSpawned(
      id: number,
      command: string,
      opts: {
        action?: string;
        background?: boolean;
        readyPattern?: string;
        pid?: number;
        label?: string;
      } = {},
    ) {
      const r = this._track(id, command, opts);
      useUiStore().consoleVisible = true;
      return r;
    },
    /** 执行一次性命令：返回 Promise 在进程退出时结算；控制台自动弹出 */
    async run(command: string): Promise<{ code: number | null; tail: string }> {
      const r = await this.spawn(command);
      return this.waitExit(r.id);
    },
    /** 挂载已 spawn 的命令并等待退出（仅一次性命令用；长驻进程绝不要 await 这个） */
    async attach(id: number, command: string): Promise<{ code: number | null; tail: string }> {
      if (!this.runs.find((r) => r.id === id)) this._track(id, command);
      return this.waitExit(id);
    },
    /** 等待某条命令退出并返回尾部输出 */
    async waitExit(id: number): Promise<{ code: number | null; tail: string }> {
      const run = this.runs.find((r) => r.id === id);
      if (run && run.status !== "running" && run.status !== "ready") {
        return { code: run.code, tail: this._tailOf(id) };
      }
      const code = await new Promise<number | null>((resolve) => {
        this._resolvers[id] = (r) => resolve(r.code);
        // 兜底：事件可能已错过（切面板/重载），每2s 查一次真实状态
        const timer = setInterval(() => {
          const r = this.runs.find((x) => x.id === id);
          if (!r || (r.status !== "running" && r.status !== "ready")) {
            clearInterval(timer);
            clearTimeout(deadline);
            resolve(r?.code ?? null);
            delete this._resolvers[id];
          }
        }, 2000);
        const deadline = setTimeout(() => {
          clearInterval(timer);
          resolve(this.runs.find((r) => r.id === id)?.code ?? null);
          delete this._resolvers[id];
        }, 600_000);
      });
      return { code, tail: this._tailOf(id) };
    },
    _tailOf(id: number): string {
      const run = this.runs.find((x) => x.id === id);
      return (run?.lines ?? [])
        .slice(-60)
        .map((l) => l.line)
        .join("\n");
    },
    /** 停止某条命令（用户显式操作；与「停止 AI」是两个语义，B1 拆开了） */
    stop(id: number) {
      const run = this.runs.find((r) => r.id === id);
      if (run) run.stoppedByUser = true;
      void api.agentKillCommand(id);
    },
    /** 兼容旧调用名 */
    kill(id: number) {
      this.stop(id);
    },
    killAll() {
      for (const r of this.runs) {
        if (r.status === "running" || r.status === "ready") this.stop(r.id);
      }
    },
    clear() {
      this.runs = this.runs.filter((r) => r.status === "running" || r.status === "ready");
    },
  },
});