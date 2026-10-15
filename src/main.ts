// 启动性能埋点：performance.now() 自页面导航起点计（PF7 分段观测）
const bootStart = performance.now();
(window as unknown as Record<string, number>)["__easyideBootStart"] = bootStart;

import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { installErrorGuard } from "./lib/errorGuard";
import "./assets/theme.css";
import "./assets/anim.css";

const app = createApp(App);
app.use(createPinia());
// 全局兜底：此前组件异常 / 未处理 Promise 拒绝完全静默，用户只看到「点了没反应」
installErrorGuard(app);

// PF7：冷启动分段埋点 —— 记录「导航→脚本执行」「→Vue 挂载完成」两段耗时。
// Rust 侧注入 __easyideTauriStart（窗口创建时刻），三段合看即可定位瓶颈：
// tauri_to_mount 大 ⇒ 瓶颈在 Rust/索引；nav_to_script 大 ⇒ 瓶颈在 JS 解析。
const navToScript = Math.round(performance.now() - bootStart);

app.mount("#app");

const bootToMount = Math.round(performance.now() - bootStart);
try {
  const tauriStart = (window as unknown as Record<string, number | undefined>)[
    "__easyideTauriStart"
  ];
  const breakdown = {
    nav_to_script_ms: navToScript,
    nav_to_mount_ms: bootToMount,
    tauri_to_mount_ms: typeof tauriStart === "number" ? bootToMount - tauriStart : null,
    at: new Date().toISOString(),
  };
  localStorage.setItem("easyide-boot-breakdown", JSON.stringify(breakdown));
  console.info("[perf] 冷启动分段", breakdown);
} catch {
  /* localStorage 不可用时忽略 */
}
