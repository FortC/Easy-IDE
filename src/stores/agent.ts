import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { AgentMessage, SessionMeta } from "../types";
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
      this.error = "";
      this.streamText = "";
      if (this.items.length === 0) {
        this.title = userText.slice(0, 24);
      }
      this.items.push({ role: "user", content: userText });
      try {
        const system = await this.buildSystem();
        for (let i = 0; i < MAX_LOOP; i++) {
          const messages: AgentMessage[] = this.items.map((it) => ({
            role: it.role,
            content: it.content,
          }));
          const text = await this.streamOnce(messages, system);
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
            if (!ok) {
              obs = "用户拒绝了该命令的执行。请调整方案或询问用户。";
              rejected = true;
            } else {
              const consoleStore = useConsoleStore();
              const { code, tail } = await consoleStore.run(call.args.command ?? "");
              obs = `[exit code: ${code ?? "null"}]\n${tail || "（无输出）"}`;
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
        this.error = String(e);
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
      if (this._streamId) void api.agentCancel(this._streamId);
      if (this.pendingCommand) this.resolveCommand(false);
      if (this.pendingEdit) this.resolveEdit(false);
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
