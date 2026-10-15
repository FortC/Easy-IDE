/**
 * 全局错误兜底。
 *
 * 背景：项目里 130+ 处 Rust 命令返回 Result<_, String>，此前只有 6 个调用点弹了
 * toast，其余大量 `catch {}` 静默吞掉 —— 用户看到的是「点了没反应」，无从排查。
 *
 * 这里兜住三类此前会彻底静默的失败：
 *   1. Vue 组件内部未捕获异常
 *   2. 未处理的 Promise 拒绝
 *   3. window 上的 error 事件
 *
 * 带频次上限，避免构建/索引类循环失败时刷屏。
 */
import type { App } from "vue";
import { useUiStore } from "../stores/ui";

const WINDOW = 10_000;
const MAX_PER_WINDOW = 3;

let hits: number[] = [];
/** 已提示过的错误摘要，用于去重（同一错误只提示一次） */
const seen = new Set<string>();

function tooMany(): boolean {
  const now = Date.now();
  hits = hits.filter((t) => now - t < WINDOW);
  if (hits.length >= MAX_PER_WINDOW) return true;
  hits.push(now);
  return false;
}

/** 从任意抛出物里提取可读摘要 */
export function summarize(err: unknown): string {
  if (err instanceof Error) return err.message || err.name;
  if (typeof err === "string") return err;
  try {
    return JSON.stringify(err).slice(0, 200);
  } catch {
    return String(err);
  }
}

function report(err: unknown, context: string) {
  const msg = summarize(err);
  // 与上一次相同且在窗口内 → 不重复提示
  const key = `${context}:${msg}`;
  if (seen.has(key) || tooMany()) {
    console.error(`[${context}]`, err);
    return;
  }
  seen.add(key);
  console.error(`[${context}]`, err);
  try {
    useUiStore().notify(`操作失败：${msg}`, "error");
  } catch {
    /* store 未就绪时只能落日志 */
  }
  // 一分钟后允许同类错误再次提示
  setTimeout(() => seen.delete(key), 60_000);
}

export function installErrorGuard(app: App) {
  app.config.errorHandler = (err, _instance, info) => {
    report(err, `渲染异常(${info})`);
  };

  window.addEventListener("error", (e) => {
    // 资源加载失败（如字体、图片）与脚本异常不同，不作为异常上报
    if (e.target !== window) return;
    report(e.error ?? e.message, "脚本异常");
  });

  window.addEventListener("unhandledrejection", (e) => {
    report(e.reason, "未处理的异步失败");
  });
}
