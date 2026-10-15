/**
 * AI 快捷动作定义（语境化）
 * 代码语境：运行 / 调试验证 / 构建 / 拉依赖 / 修复报错 / 解读代码 / 跑测试
 * 笔记语境：总结 / 续写 / 生成标签 / 转小计 / 提炼大纲
 *
 * 动作集由 ui.context 决定，在 AgentPanel 顶部以 chips 呈现（AI2：消灭 RunBar 第二入口）。
 * RunBar 保留同一份定义，仅在代码语境作为快速触发条使用。
 */
import type { WorkContext } from "../types";

export type DevAction = "run" | "debug" | "build" | "deps" | "fix" | "explain" | "test";
export type NoteAction = "summarize" | "continue" | "autotag" | "toJot" | "outline";

export interface ActionDef {
  key: DevAction | NoteAction;
  icon: string;
  /** i18n 键（短标签） */
  label: string;
  /** i18n 键（悬停说明） */
  title: string;
}

/** 可沉淀配方的动作（纯对话场景不沉淀） */
export const RECIPE_ACTIONS: DevAction[] = ["run", "debug", "build", "deps", "test"];

/** 代码语境动作 */
export const DEV_ACTIONS: ActionDef[] = [
  { key: "run", icon: "play", label: "rb2.run", title: "rb2.runTitle" },
  { key: "debug", icon: "bug", label: "rb2.debug", title: "rb2.debugTitle" },
  { key: "build", icon: "hammer", label: "rb2.build", title: "rb2.buildTitle" },
  { key: "deps", icon: "package", label: "rb2.deps", title: "rb2.depsTitle" },
  { key: "fix", icon: "wrench", label: "rb2.fix", title: "rb2.fixTitle" },
  { key: "explain", icon: "book", label: "rb2.explain", title: "rb2.explainTitle" },
  { key: "test", icon: "check-square", label: "rb2.test", title: "rb2.testTitle" },
];

/** 笔记语境动作 */
export const NOTE_ACTIONS: ActionDef[] = [
  { key: "summarize", icon: "list-ordered", label: "na.summarize", title: "na.summarizeTitle" },
  { key: "continue", icon: "pencil", label: "na.continue", title: "na.continueTitle" },
  { key: "autotag", icon: "tag", label: "na.autotag", title: "na.autotagTitle" },
  { key: "toJot", icon: "clock", label: "na.toJot", title: "na.toJotTitle" },
  { key: "outline", icon: "list-ordered", label: "na.outline", title: "na.outlineTitle" },
];

/** 按语境取动作集 */
export function actionsOf(ctx: WorkContext): ActionDef[] {
  return ctx === "dev" ? DEV_ACTIONS : NOTE_ACTIONS;
}

/** 笔记语境动作的提示词构造 */
export function notePrompt(
  key: NoteAction,
  opts: { title: string; content: string; selection: string },
): string {
  const sel = opts.selection ? opts.selection.slice(0, 4000) : "";
  const scope = sel ? "以下选中的段落：\n\n" + sel : "当前笔记全文：\n\n" + opts.content.slice(0, 8000);
  switch (key) {
    case "summarize":
      return `请为${sel ? "选中的段落" : "当前笔记"}做要点总结，用条目列出核心结论（不超过 5 条），只输出总结本身。\n\n${scope}`;
    case "continue":
      return `请接着当前内容自然续写（保持原有语气与结构，控制在 300 字内）。只输出续写内容，不要解释。\n\n${scope}`;
    case "autotag":
      return `请为当前笔记推荐 3-6 个标签，输出为逗号分隔的纯标签名（如：java,spring,缓存），不要任何其他文字。\n\n标题：${opts.title}\n\n${scope}`;
    case "toJot":
      return `请把当前笔记压缩成一条小计（不超过 80 字的时间线记录），只输出小计正文。\n\n${scope}`;
    case "outline":
      return `请为当前笔记生成层级大纲（用 - 与缩进表示），只输出大纲。\n\n${scope}`;
  }
  // 穷尽兜底
  return "";
}

/** 代码语境动作的提示词构造（配方失效时附失效提示，引导 AI 增量修正而非盲执行） */
export function devPrompt(
  key: DevAction,
  opts: {
    file: string;
    consoleTail: string;
    staleNote: string;
  },
): string {
  const { staleNote } = opts;
  switch (key) {
    case "run":
      return `${staleNote}请运行当前项目：先用工具查看项目根目录与构建文件（pom.xml / build.gradle / package.json 等）判断项目类型与入口，确定运行命令后用 run_command 执行（会请求用户确认），执行后把关键输出（启动成功标志 / 端口 / 报错）汇报给我。`;
    case "debug":
      return `${staleNote}请以调试模式运行当前项目：先判断项目类型，选择带调试参数的运行方式（如 JVM 加 -Ddebug、Spring Boot 加 --debug，或打印详细日志的配置），用 run_command 执行（会请求用户确认），然后重点汇总日志里的 ERROR 与 WARN 及其原因。若控制台已有运行中的进程请先说明。`;
    case "build":
      return `${staleNote}请构建当前项目（跳过测试，如 mvn package -DskipTests 或 npm run build）：先确认构建方式，用 run_command 执行（会请求用户确认），结束后汇报构建结果与产物路径。`;
    case "deps":
      return `${staleNote}请拉取/刷新当前项目依赖（如 mvn dependency:resolve 或 npm install）：先确认包管理方式，用 run_command 执行（会请求用户确认），结束后汇报结果（新增/变更的依赖或失败原因）。`;
    case "fix":
      return `请帮我修复最近的报错。先看下面的控制台输出定位原因，需要时用工具（read_file / search_symbols / search_text）查看相关代码，然后给出具体修改方案（文件、位置、改法）；如果修改很小且明确，说明改完后我可以手动应用。

最近控制台输出：
${opts.consoleTail || "（暂无输出，请先运行一次项目）"}`;
    case "explain":
      return "请解释当前项目的结构：用工具浏览根目录、关键构建文件与源码目录，给出模块划分、技术栈（框架与版本）、配置要点与启动方式的简明项目地图。";
    case "test":
      return `请为当前打开的文件生成单元测试骨架（只生成代码，不执行）：先用 read_file 查看该文件，再给出完整的测试类代码与依赖说明。当前文件：${opts.file || "（未打开文件）"}`;
  }
  // 穷尽兜底：新增动作时不会静默返回空串
  return "";
}

/** 配方失效提示（失效即回落 AI 增量确认，不盲执行过期命令） */
export function staleNoteOf(
  key: string,
  statusOf: (k: DevAction) => { valid: boolean; command: string; changed: string[] } | null,
): string {
  if (!RECIPE_ACTIONS.includes(key as DevAction)) return "";
  const st = statusOf(key as DevAction);
  if (!st || st.valid) return "";
  return (
    `注意：本项目已有「${key}」配方但已失效（变化文件：${st.changed.join(", ") || "未知"}），` +
    `原命令：\`${st.command}\`。\n请优先基于原命令做增量确认或最小修正后执行，` +
    `成功后我会重新沉淀配方。\n\n`
  );
}
