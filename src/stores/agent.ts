import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { AgentMessage, CommandLogEntry, SessionMeta } from "../types";
import { useConsoleStore } from "./console";
import { useEditorStore } from "./editor";
import { useUiStore } from "./ui";
import { useVaultStore } from "./vault";

/** 展示用消息（带工具调用元数据） */
export interface ChatItem {
  role: "user" | "assistant";
  content: string;
  /** 该条 assistant 消息触发的工具调用（展示块） */
  tool?: { name: string; args: Record<string, string>; result: string; rejected?: boolean };
}

const MAX_LOOP = 8;

/**
 * 长驻进程识别（B1）
 *
 * 判定依据是「命令是否启动一个不会自己退出的服务」，而不是精确匹配某条命令 ——
 * 用户的启动命令千变万化（`mvn spring-boot:run` / `java -jar` / `npm run dev`），
 * 但「服务型」子命令集合是稳定的。
 *
 * 误判代价分析：把一次性命令误判为长驻 → AI 不等结果，观察里没有退出码，
 * 但进程照常跑完，只是 AI 少看了结果；反之（长驻误判为一次性）→ 卡死，必须避免。
 */
const LONG_LIVED_TOKENS = [
  "spring-boot:run",
  "spring-boot:devtools",
  "boot:run",
  "tomcat:run",
  "jetty:run",
  "quarkus:dev",
  "bootRun",
  "npm run dev",
  "pnpm dev",
  "yarn dev",
  "vite",
  "webpack serve",
  "ng serve",
  "next dev",
  "nuxt dev",
  "uvicorn",
  "flask run",
  "manage.py runserver",
  "nodemon",
  "nodemon ",
  "cargo watch",
];

/** 明确一次性、不该当长驻的（放在长驻表之后判定，避免 `vite build` 被误判） */
const ONE_SHOT_TOKENS = ["vite build", "next build", "ng build", "flask build"];

/** 判断一条命令是否是「启动后长期运行」的服务 */
export function isLongLived(command: string): boolean {
  const c = command.toLowerCase();
  // -c / --watch 等一次性语义先排除
  if (ONE_SHOT_TOKENS.some((t) => c.includes(t))) return false;
  return LONG_LIVED_TOKENS.some((t) => c.includes(t.toLowerCase()));
}

/**
 * 就绪检测正则表：命中即把运行态从「启动中」升级为「已就绪」（B2 的绿色常亮）。
 * 通用而非项目特定，避免为每个框架各写一份。
 */
const READY_PATTERNS = [
  // Spring Boot / Tomcat
  "Started\\s+\\w+.*in\\s+[\\d.]+\\s*seconds",
  "Tomcat started on port",
  // Node / 前端 dev server
  "ready in \\d+",
  "Local:\\s+http",
  "Network:\\s+http",
  "compiled successfully",
  "webpack compiled",
  "vite v\\d+.*ready",
  // Python
  "Uvicorn running on",
  "Running on http",
  "Development Server is ready",
  // 通用兜底
  "Started .* on port \\d+",
  "listening on port \\d+",
];

function matchesReady(pattern: string, command: string): boolean {
  // 命令里没提端口就别套端口正则，避免误报
  if (/port|端口/i.test(command) || pattern.includes("seconds") || pattern.includes("Local:")) {
    return true;
  }
  return /vite|dev|run|serve|start/i.test(command);
}

/** AI Agent 中枢（前端编排：流式 → 解析工具调用 → 确认门 → 执行 → 观察 → 继续） */
export const useAgentStore = defineStore("agent", {
  state: () => ({
    sessionId: "",
    title: "",
    items: [] as ChatItem[],
    sessions: [] as SessionMeta[],
    streaming: false,
    streamText: "",
    error: "",
    /** 最近一轮 run() 里执行过的命令记录（配方捕获候选） */
    lastCommandLog: [] as CommandLogEntry[],
    /**
     * 选区上下文（AI5）：编辑器选中代码后自动挂载，随下一次提问一起发给 AI。
     * 消灭「手动复制粘贴」这一步，也避免 AI 自己 read_file 找回来（多轮往返 + 耗 token）。
     */
    contextSelection: null as {
      /** 文件名（相对路径末段，用于显示） */
      name: string;
      /** 相对路径（供 AI 定位） */
      path: string;
      /** 选中的文本 */
      text: string;
      /** 起始行（0-based） */
      from: number;
      /** 结束行（0-based） */
      to: number;
    } | null,
    /** 等待用户确认的命令（确认门） */
    pendingCommand: null as {
      command: string;
      thought: string;
      resolve: (ok: boolean) => void;
    } | null,
    /** 等待用户批准的文件修改提案（write_file 确认门） */
    pendingEdit: null as {
      path: string;
      description: string;
      diff: string;
      newContent: string;
      resolve: (ok: boolean) => void;
    } | null,
    _streamId: 0,
    /** 正在跑的一次性命令 id 集合（长驻进程不登记：它不该随 AI 停止而被杀） */
    _cmdIds: new Set<number>(),
    /** 本轮后台启动的长驻进程（供 UI 显示 pid / 提供停止入口） */
    lastStarted: null as { id: number; pid: number; command: string } | null,
    /** 用户点了「停止」：主循环每步检查并退出 */
    stopped: false,
    _unsubs: [] as (() => void)[],
  }),
  actions: {
    /** 事件接线（App.vue 挂载时调用一次） */
    bindEvents() {
      void api.onAgentDelta((e) => {
        if (e.id === this._streamId) this.streamText += e.text;
      });
      this._unsubs.push(() => {
        /* onAgentDelta 无法单独解绑（全局流），保留即可 */
      });
    },

    /** 系统提示词：工具协议 + 项目 AI 规则（AGENTS.md，P2 生成）+ 工作区信息 */
    async buildSystem(): Promise<string> {
      const vault = useVaultStore();
      let rules = "";
      try {
        rules = await api.readTextFile("AGENTS.md");
      } catch {
        /* 无 AGENTS.md 时跳过 */
      }
      const rulesBlock = rules.trim()
        ? `\n\n# 项目 AI 协作规则（来自项目 AGENTS.md）\n${rules.trim()}`
        : "";
      return `你是 EasyIDE（代码 IDE）里的 AI 助手，帮助用户开发当前项目。工作区：${vault.root || "(未打开)"}。
当前打开的文件会由用户消息附带。

# 可用工具
当你需要执行操作时，回复中输出且仅输出一个 JSON 代码块（不要输出其它内容）：
\`\`\`json
{"tool": "<工具名>", "args": {...}, "thought": "一句话说明目的"}
\`\`\`
工具清单：
- read_file：{ "path": "相对路径" } —— 读工作区文本文件
- list_dir：{ "path": "相对路径或空字符串" } —— 列目录
- search_text：{ "query": "关键字" } —— 全文搜索代码与文档
- search_symbols：{ "query": "类/方法名" } —— 搜索符号（类/方法/函数/字段）
- run_command：{ "command": "命令行" } —— 在项目根执行命令（mvn/java/npm/git 等，需用户确认）
- write_file：{ "path": "相对路径", "content": "完整的新文件内容", "description": "一句话说明改动" } —— 修改文件（用户会看到 diff 预览并批准）

规则：
1. 一次只调用一个工具；工具结果会以「[工具结果]」开头的用户消息返回。
2. run_command 与 write_file 都会请求用户确认，被拒绝时换思路或询问用户。
3. write_file 必须输出文件的完整内容（不是片段），基于 read_file 读到的原文做最小修改。
3. 信息足够后，直接输出最终答复（普通 Markdown 文本，不带 JSON 块）。
4. 回答用简体中文，简洁直接。${rulesBlock}`;
    },

    /** 主入口：跑一轮 Agent 循环 */
    async run(userText: string) {
      if (this.streaming) return;
      this.streaming = true;
      this.stopped = false;
      this.error = "";
      this.streamText = "";
      this.lastCommandLog = [];
      if (this.items.length === 0) {
        this.title = userText.slice(0, 24);
      }

      // AI5：选区上下文注入 —— 把用户选中的代码作为结构化上下文附在提问前，
      // AI 无需再 read_file 找回来（省一轮往返，也省 token）
      const ctx = this.contextSelection;
      const effectiveText = ctx
        ? `[选区上下文] 文件：${ctx.path}（第 ${ctx.from + 1}-${ctx.to + 1} 行）\n\`\`\`\n${ctx.text}\n\`\`\`\n\n问题：${userText}`
        : userText;
      this.contextSelection = null; // 注入一次即清空，避免同一选区反复污染后续提问
      this.items.push({ role: "user", content: effectiveText });
      try {
        const system = await this.buildSystem();
        for (let i = 0; i < MAX_LOOP; i++) {
          if (this.stopped) break;
          const messages: AgentMessage[] = this.items.map((it) => ({
            role: it.role,
            content: it.content,
          }));
          const text = await this.streamOnce(messages, system);
          if (this.stopped) break;
          const call = parseToolCall(text);
          if (!call) {
            this.items.push({ role: "assistant", content: text });
            break;
          }
          // 工具调用：run_command 过确认门
          let obs: string;
          let rejected = false;
          if (call.tool === "run_command") {
            const ok = await this.gateCommand(call.args.command ?? "", call.thought);
            if (this.stopped) break;
            if (!ok) {
              obs = "用户拒绝了该命令的执行。请调整方案或询问用户。";
              rejected = true;
            } else {
              const consoleStore = useConsoleStore();
              const command = call.args.command ?? "";
              // B1：长驻命令（启动服务类）绝不能 await 到进程退出 —— 那会让
              // AI 永久卡在 streaming，用户只能点「停止」，而「停止」又会杀进程。
              // 这里 spawn 后立即拿pid 返回，AI 循环正常收尾。
              if (isLongLived(command)) {
                const r = await consoleStore.spawn(command, {
                  background: true,
                  readyPattern: READY_PATTERNS.find((p) => matchesReady(p, command)),
                });
                this.lastStarted = { id: r.id, pid: r.pid, command };
                // B6：AI 启动的 Java 长驻服务同样纳入热部署编排
                const { useHotReloadStore } = await import("./hotReload");
                useHotReloadStore().watch(r.id, null, command);
                obs =
                  `[已在后台启动] pid=${r.pid} cmd_id=${r.id}。\n` +
                  `输出持续写入控制台面板，进程不会随本轮对话结束而停止。` +
                  `用户可在代码区头部的运行条点「停止」来结束它。` +
                  `（若为 Java 服务，已开启热更新：改代码会自动编译并重启，编译失败则保持旧进程运行。）`;
              } else {
                const { id } = await consoleStore.spawn(command);
                this._cmdIds.add(id);
                const { code, tail } = await consoleStore.waitExit(id);
                this._cmdIds.delete(id);
                this.lastCommandLog.push({ command, code, tail });
                obs = `[exit code: ${code ?? "null"}]\n${tail || "（无输出）"}`;
              }
            }
          } else {
            obs = await execTool(call.tool, call.args);
          }
          this.items.push({
            role: "assistant",
            content: text,
            tool: {
              name: call.tool,
              args: call.args,
              result: obs,
              rejected,
            },
          });
          this.items.push({ role: "user", content: `[工具结果]\n${obs}` });
          this.streamText = "";
        }
        await this.saveSession();
      } catch (e) {
        if (!this.stopped) this.error = String(e);
      } finally {
        this.streaming = false;
        this.streamText = "";
      }
    },

    /** 单次流式请求（Promise 包装事件） */
    async streamOnce(messages: AgentMessage[], system: string): Promise<string> {
      return new Promise((resolve, reject) => {
        void (async () => {
          try {
            const id = await api.agentChatStream(messages, system);
            this._streamId = id;
            const offDone = await api.onAgentDone((e) => {
              if (e.id !== id) return;
              offDone();
              offError();
              resolve(e.text);
            });
            const offError = await api.onAgentError((e) => {
              if (e.id !== id) return;
              offDone();
              offError();
              reject(("error" in e && e.error) || "AI 请求失败");
            });
          } catch (e) {
            reject(e);
          }
        })();
      });
    },

    /** 确认门：暂停循环等待用户点击 */
    gateCommand(command: string, thought: string): Promise<boolean> {
      return new Promise((resolve) => {
        this.pendingCommand = { command, thought, resolve };
      });
    },
    resolveCommand(ok: boolean) {
      this.pendingCommand?.resolve(ok);
      this.pendingCommand = null;
    },
    /** 文件修改提案确认门：diff 预览 → 批准落盘 → 编辑器同步 */
    async gateEdit(path: string, newContent: string, description: string): Promise<boolean> {
      let diff = "";
      try {
        diff = await api.diffPreview(path, newContent);
        if (!diff) diff = "（内容没有变化）";
      } catch (e) {
        diff = `（无法生成 diff 预览：${String(e)}）`;
      }
      const ok = await new Promise<boolean>((resolve) => {
        this.pendingEdit = { path, description, diff, newContent, resolve };
      });
      if (ok) {
        try {
          await api.writeTextFile(path, newContent);
          useEditorStore().reloadDoc(path, newContent);
        } catch (e) {
          window.setTimeout(() => {
            this.error = `写入文件失败：${String(e)}`;
          }, 0);
          return false;
        }
      }
      return ok;
    },
    resolveEdit(ok: boolean) {
      const p = this.pendingEdit;
      this.pendingEdit = null;
      p?.resolve(ok);
    },
    stop() {
      // 停整条 AI 链：置停止标志（主循环逐步退出）+ 取消当前 AI 流 + 关闭确认门。
      //
      // ⚠️ B1 关键修复：这里**不再杀进程**。
      // 以前 stop() 会调 agentKillCommand，导致「停止 AI」= 「杀掉正在运行的项目」，
      // 而长驻进程又让 AI 永远处于 streaming，用户唯一出路就是点停止 → 项目被误杀。
      // 现在「停止 AI」与「停止进程」彻底分离：进程只在用户点运行条的 ⏹ 时才停。
      this.stopped = true;
      this.streaming = false;
      this.streamText = "";
      if (this._streamId) {
        void api.agentCancel(this._streamId);
        this._streamId = 0;
      }
      if (this.pendingCommand) this.resolveCommand(false);
      if (this.pendingEdit) this.resolveEdit(false);
    },

    /**
     * 停止某个动作对应的进程（B2 的 ⏹ 按钮专用）。
     * 与 stop() 语义分离：只停进程，不动 AI 会话。
     */
    stopProcess(id: number) {
      useConsoleStore().stop(id);
    },

    // ---- 会话管理 ----
    async refreshSessions() {
      try {
        this.sessions = await api.agentSessionsList();
      } catch {
        this.sessions = [];
      }
    },
    newSession() {
      this.stop();
      this.sessionId = `s${Date.now()}`;
      this.title = "";
      this.items = [];
      this.error = "";
    },
    async loadSession(id: string) {
      this.stop();
      const doc = await api.agentSessionLoad(id);
      this.sessionId = doc.id;
      this.title = doc.title;
      this.items = doc.messages.map((m) => ({ role: m.role, content: m.content }));
    },
    async saveSession() {
      if (this.items.length < 2 || !this.sessionId) {
        this.sessionId = this.sessionId || `s${Date.now()}`;
      }
      try {
        await api.agentSessionSave({
          id: this.sessionId,
          title: this.title || "会话",
          updated: new Date().toISOString(),
          messages: this.items.map((it) => ({ role: it.role, content: it.content })),
        });
        await this.refreshSessions();
      } catch {
        /* 保存失败不影响对话 */
      }
    },
    async deleteSession(id: string) {
      await api.agentSessionDelete(id);
      if (id === this.sessionId) this.newSession();
      await this.refreshSessions();
    },
    /** 打开右侧 AI 面板（预设场景/编辑器入口用） */
    openPanel() {
      const ui = useUiStore();
      ui.rightVisible = true;
      ui.rightTab = "ai";
    },
    /** 挂载选区上下文（由 SourceEditor 的 selectionSet 驱动） */
    setSelectionContext(ctx: {
      name: string;
      path: string;
      text: string;
      from: number;
      to: number;
    } | null) {
      // 空选区或超长选区不挂载（超长多半是全选，AI 上下文反而被噪音淹没）
      if (!ctx || !ctx.text.trim() || ctx.text.length > 20000) {
        if (!ctx || !ctx.text.trim()) this.contextSelection = null;
        return;
      }
      this.contextSelection = ctx;
    },
    /** 清除选区上下文（用户点 × 或已发送） */
    clearSelectionContext() {
      this.contextSelection = null;
    },
  },
});

/** 解析模型回复中的工具调用 JSON 块 */
export function parseToolCall(text: string): {
  tool: string;
  args: Record<string, string>;
  thought: string;
} | null {
  const fence = /```(?:json)?\s*([\s\S]*?)```/.exec(text);
  const candidates = fence ? [fence[1]] : [];
  const trimmed = text.trim();
  if (trimmed.startsWith("{")) candidates.push(trimmed);
  for (const c of candidates) {
    try {
      const v = JSON.parse(c.trim());
      if (v && typeof v.tool === "string") {
        const args: Record<string, string> = {};
        if (v.args && typeof v.args === "object") {
          for (const [k, val] of Object.entries(v.args)) {
            args[k] = String(val ?? "");
          }
        }
        return { tool: v.tool, args, thought: String(v.thought ?? "") };
      }
    } catch {
      /* 非严格 JSON，跳过 */
    }
  }
  return null;
}

/** 执行只读工具（run_command 在 store 内单独处理） */
async function execTool(tool: string, args: Record<string, string>): Promise<string> {
  try {
    switch (tool) {
      case "read_file": {
        const content = await api.readTextFile(args.path ?? "");
        const capped =
          content.length > 6000 ? content.slice(0, 6000) + "\n…（已截断）" : content;
        return capped || "（空文件）";
      }
      case "list_dir": {
        const list = await api.listDir(args.path || null);
        return (
          list
            .map((e) => `${e.is_dir ? "[目录]" : "[文件]"} ${e.name}`)
            .join("\n") || "（空目录）"
        );
      }
      case "search_text": {
        const hits = await api.codeTextSearch(args.query ?? "", false, 30);
        return (
          hits.map((h) => `${h.path}:${h.line_no} ${h.text}`).join("\n") || "（无结果）"
        );
      }
      case "search_symbols": {
        const hits = await api.codeSymbolSearch(args.query ?? "", null, 30);
        return (
          hits
            .map((h) => `${h.kind} ${h.name} ${h.path}:${h.line}`)
            .join("\n") || "（无结果）"
        );
      }
      default:
        return `未知工具：${tool}`;
    }
  } catch (e) {
    return `工具执行出错：${String(e)}`;
  }
}
