#!/usr/bin/env node
// 首屏包体预算校验 —— 让 AGENTS.md 的性能红线可自动判定，而不是靠人记。
// 首屏定义：dist/index.html 直接引用的 entry chunk（模块脚本 + 样式表）。
// 懒加载 chunk 不计入首屏预算（它们由 defineAsyncComponent / 动态 import 触发）。
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { gzipSync } from "node:zlib";

const root = resolve(new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"));
const dist = join(root, "dist");
const budget = JSON.parse(readFileSync(join(root, "docs", "performance-budget.json"), "utf8"));

const kb = (n) => (n / 1024).toFixed(1).padStart(7) + " KB";
const limits = budget.budget;
const target = budget.baseline ?? {};

function readIndex() {
  const html = readFileSync(join(dist, "index.html"), "utf8");
  const js = [...html.matchAll(/<script[^>]+src="([^"]+\.js)"/g)].map((m) => m[1]);
  const css = [...html.matchAll(/<link[^>]+href="([^"]+\.css)"/g)].map((m) => m[1]);
  return { js, css };
}

const { js: entryJs, css: entryCss } = readIndex();
if (!entryJs.length) {
  console.error("✗ dist/index.html 里没有找到 entry 脚本，请先执行 npm run build");
  process.exit(1);
}

let jsTotal = 0;
let cssTotal = 0;
const rows = [];

for (const rel of [...entryJs, ...entryCss]) {
  const p = join(dist, rel.replace(/^\.?\//, ""));
  if (!statSync(p, { throwIfNoEntry: false })) continue;
  const buf = readFileSync(p);
  const name = rel.split("/").pop();
  if (name.endsWith(".css")) cssTotal += buf.length;
  else jsTotal += buf.length;
  rows.push({ name, raw: buf.length, gz: gzipSync(buf).length });
}

console.log("\n  首屏包体（dist/index.html 直接引用）\n  " + "-".repeat(46));
for (const r of rows.sort((a, b) => b.raw - a.raw)) {
  console.log(`  ${r.name.padEnd(30)} ${kb(r.raw)}   gzip ${kb(r.gz)}`);
}
console.log("  " + "-".repeat(46));
console.log(`  合计 JS ${kb(jsTotal)}   合计 CSS ${kb(cssTotal)}\n`);

let failed = false;
const check = (label, actual, limit, goal) => {
  const ok = actual <= limit;
  if (!ok) failed = true;
  const mark = goal ? (actual <= goal ? "·优于瘦身前基线" : `·较基线回退 ${kb(actual - goal).trim()}`) : "";
  console.log(`  ${ok ? "✓" : "✗"} ${label.padEnd(18)} ${kb(actual).trim()} / 预算 ${kb(limit).trim()} ${mark}`);
};
check("首屏 JS", jsTotal, limits.firstScreenJs, target.firstScreenJs);
check("首屏 CSS", cssTotal, limits.firstScreenCss, target.firstScreenCss);

// vendor 块虽是首屏必载，但单独设更宽松的天花板，防止编辑器内核无声膨胀
const files = readdirSync(join(dist, "assets")).filter((f) => f.endsWith(".js"));
for (const [chunk, limit] of Object.entries(budget.vendorJs ?? {})) {
  const hit = files.find((f) => f.startsWith(chunk));
  if (!hit) continue;
  check(chunk, statSync(join(dist, "assets", hit)).size, limit);
}

console.log(failed ? "\n  ✗ 首屏包体超预算\n" : "\n  ✓ 首屏包体在预算内\n");
process.exit(failed ? 1 : 0);
