/**
 * B6：热部署编排。
 *
 * 先说清边界，避免误解：真 JRebel 靠 JVM 字节码增强，**这里不做**，也做不到等价。
 * 做的是用户真实诉求 ——「改了代码不想手动重启」，两条路径：
 *
 *  - **档位 3（非 JVM）**：Node 的 `vite dev` / Python 的 `uvicorn --reload` 本来就有
 *    热更新能力。这里只需保证启动它们时用后台 spawn + 就绪探测（那是 B1 已建好的），
 *    所以**运行时零改动**，只在启用热部署时补一条「后端就绪探针」，
 *    让用户一眼看到「改完自动生效中」而不是靠猜。
 *  - **档位 1（Java）**：改动源码 → 触发编译 → 编译成功才重启，失败则**保持旧进程不动**。
 *    这是本文件的主要工作。关键约束：
 *      · 编译失败绝不能杀掉正在跑的进程（否则改错一个字服务就挂了）；
 *      · 必须去抖，否则用户连续保存 5 次会重启 5 次；
 *      · 同一模块的连续变更必须合并成一次重启；
 *      · 重启要走 B1 的 spawn 语义，不要退回 attach（否则这里自己就会卡死）。
 */

import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import type { PomModuleNode } from "../types";
import { useConsoleStore } from "./console";
import { useUiStore } from "./ui";

/** 去抖窗口：连续保存合并成一次重启 */
const DEBOUNCE_MS = 1500;
/** 参与热部署的源码扩展名（其余如 pom.xml、配置交给各自链路） */
const SOURCE_EXT = [
  ".java", ".kt", ".xml", ".yml", ".yaml", ".properties",
  ".js", ".jsx", ".ts", ".tsx", ".vue", ".py", ".go", ".rs",
];

function isSource(rel: string): boolean {
  const lower = rel.toLowerCase();
  return SOURCE_EXT.some((e) => lower.endsWith(e));
}

/** 从变更文件路径推出所属 Maven 模块：取命中的最深模块前缀（最具体优先） */
function moduleOf(rel: string, tree: PomModuleNode[]): string | null {
  let best: string | null = null;
  for (const m of tree) {
    if (m.rel_path === ".") continue; // 根模块兜底，命中任何非根模块时都不用它
    const hit = rel.startsWith(m.rel_path + "/");
    if (hit && (best === null || m.rel_path.length > best.length)) best = m.rel_path;
  }
  if (best) return best;
  // 根 pom 的子树之外的路径（如顶层 src/main）：仍归属根模块
  return tree.some((m) => m.rel_path === ".") ? "." : null;
}

export interface HotReloadState {
  /** 总开关 */
  enabled: boolean;
  /** Java 自动重启（档位 1） */
  javaRestart: boolean;
  /** 非 JVM 进程的热更新提示（档位 3） */
  nodeHot: boolean;
  /** 正在编译 */
  compiling: boolean;
  /** 上次编译结果（用于在 RunBar 上给用户明确反馈） */
  lastBuildOk: boolean | null;
  lastBuildMsg: string;
  /** 已被编排接管的进程 id（避免重复接管同一条） */
  _managed: Record<number, { module: string; command: string }>;
  _timer: ReturnType<typeof setTimeout> | null;
}

export const useHotReloadStore = defineStore("hotReload", {
  state: (): HotReloadState => ({
    enabled: true,
    javaRestart: true,
    nodeHot: true,
    compiling: false,
    lastBuildOk: null,
    lastBuildMsg: "",
    _managed: {},
    _timer: null,
  }),

  getters: {
    /** 有没有正在被编排的进程（RunBar 用它显示「自动重启已开」） */
    managedCount: (s) => Object.keys(s._managed).length,
  },

  actions: {
    /**
     * 接管一个长驻进程：后续源码变更会触发「编译 → 重启它」。
     * 只接管长驻进程 —— 一次性命令（build/test）挂了没有重启意义。
     *
     * `module` 为 "*" 表示「不限模块」：命令里没写清模块（如根目录 `mvn
     * spring-boot:run`）时，任何模块的改动都可以触发它的重启。多模块项目下
     * 这种精度有限，但比要求用户先在按钮里声明模块更实用 —— 按钮是随手存的，
     * 没人会先去设置里填模块名。
     */
    watch(runId: number, module: string | null, command: string) {
      if (!this.enabled || !this.javaRestart) return;
      // 只接管「看起来是 Java 服务」的命令，避免把 vite dev 之类也拖进 Maven 编译链路
      if (!this._looksJava(command)) return;
      this._managed[runId] = { module: module ?? "*", command };
    },

    /** 进程退出后解除接管，否则会对一个已经不存在的进程反复重启 */
    unwatch(runId: number) {
      delete this._managed[runId];
    },

    /** 命令是否像 Java 服务（只把这类纳入 Maven 编译→重启链路） */
    _looksJava(command: string): boolean {
      const c = command.toLowerCase();
      return (
        c.includes("spring-boot:run") ||
        c.includes("spring-boot:") ||
        c.includes("java -jar") ||
        c.includes("java -cp") ||
        c.includes("exec:java")
      );
    },

    /**
     * 源码变更入口（App.vue 在 vault-changed 里调用）。
     * 非源码变更、被排除的目录直接返回；这里只负责排期，不阻塞调用方。
     */
    onCodeChanged(changed: string[]) {
      if (!this.enabled || !this.javaRestart) return;
      const src = changed.filter((r) => isSource(r) && !this._ignored(r));
      if (!src.length) return;
      this._schedule(src);
    },

    /** 目标/构建产物目录不该触发重启（否则一次编译会引发自激重启循环） */
    _ignored(rel: string): boolean {
      const lower = rel.toLowerCase();
      return (
        lower.includes("/target/") ||
        lower.startsWith("target/") ||
        lower.includes("/node_modules/") ||
        lower.startsWith("node_modules/") ||
        lower.includes("/dist/") ||
        lower.startsWith("dist/") ||
        lower.includes("/build/") ||
        lower.includes("/.git/") ||
        lower.includes("/.svn/")
      );
    },

    /** 去抖排期：窗口内所有变更合并成一次 */
    _schedule(files: string[]) {
      if (this._timer) clearTimeout(this._timer);
      this._timer = setTimeout(() => {
        this._timer = null;
        void this._rebuildAndRestart(files);
      }, DEBOUNCE_MS);
    },

    async _rebuildAndRestart(files: string[]) {
      const consoleStore = useConsoleStore();
      const ui = useUiStore();
      const entries = Object.entries(this._managed);
      if (!entries.length) return;
      // 已在编译中说明上一轮还没跑完，本轮直接并入下一轮，不排队叠加
      if (this.compiling) {
        this._schedule(files);
        return;
      }

      let tree: PomModuleNode[] = [];
      try {
        tree = await api.mavenModuleTree();
      } catch {
        // 非 Maven 项目：拿不到模块树就没法定位该编译哪个模块，直接放弃编排（不猜）
        return;
      }

      // 按模块归组：一个模块只编译重启一次
      const byModule = new Map<string, string[]>();
      for (const f of files) {
        const m = moduleOf(f, tree);
        if (!m) continue;
        const arr = byModule.get(m) ?? [];
        arr.push(f);
        byModule.set(m, arr);
      }
      if (!byModule.size) return;

      this.compiling = true;
      try {
        for (const [module, mods] of byModule) {
          const runIds = entries
            .filter(([, v]) => v.module === module || v.module === "*")
            .map(([k]) => Number(k));
          if (!runIds.length) continue;
          await this._compileThenRestart(module, mods, runIds, consoleStore, ui);
        }
      } finally {
        this.compiling = false;
      }
    },

    async _compileThenRestart(
      module: string,
      files: string[],
      runIds: number[],
      consoleStore: ReturnType<typeof useConsoleStore>,
      ui: ReturnType<typeof useUiStore>,
    ) {
      const ui2 = ui;
      const cwd = module === "." ? "" : ` -f ${module}/pom.xml`;
      const cmd = `mvn -q -B compile${cwd}`;
      let code: number | null = null;
      try {
        const r = await consoleStore.run(cmd);
        code = r.code;
      } catch (e) {
        // 编译命令本身启动失败（Maven 不存在等）：不动旧进程，只报错
        this.lastBuildOk = false;
        this.lastBuildMsg = String(e);
        ui2.notify(this.lastBuildMsg, "error");
        return;
      }

      if (code !== 0) {
        // 关键约束：编译失败**不动正在跑的进程**。老进程还在服务，
        // 用户修好代码再次保存就会自动重编译重启，全程不用手动干预。
        this.lastBuildOk = false;
        this.lastBuildMsg = `编译失败（${files.length} 个文件），已保持旧进程运行`;
        ui2.notify(this.lastBuildMsg, "error");
        return;
      }

      this.lastBuildOk = true;
      this.lastBuildMsg = "编译通过，正在重启";
      for (const id of runIds) {
        const run = consoleStore.runs.find((r) => r.id === id);
        if (!run) continue;
        // 记下原绑定：重注册时要用它，否则 "*"（不限模块）会被窄化成当前模块
        const bound = this._managed[id]?.module ?? module;
        consoleStore.stop(id);
        // 等进程真正退出再拉起，否则端口还占着，新进程启动必失败
        await consoleStore.waitExit(id);
        this.unwatch(id);
        try {
          const fresh = await consoleStore.spawn(run.command, {
            action: run.action,
            background: true,
            readyPattern: run.readyPattern,
          });
          consoleStore.trackSpawned(fresh.id, run.command, {
            action: run.action,
            background: true,
            readyPattern: run.readyPattern,
            label: run.label,
          });
          this.watch(fresh.id, bound, run.command);
        } catch (e) {
          ui2.notify(`重启失败：${e}`, "error");
          return;
        }
      }
      ui2.notify(this.lastBuildMsg, "success");
    },

    /** 关闭时清掉待执行的定时器，避免离开工作区后还在编译 */
    dispose() {
      if (this._timer) clearTimeout(this._timer);
      this._timer = null;
      this._managed = {};
    },
  },
});
