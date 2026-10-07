// 启动性能埋点：performance.now() 自页面导航起点计
const bootStart = performance.now();
(window as unknown as Record<string, number>)["__easyideBootStart"] = bootStart;

import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./assets/theme.css";

createApp(App).use(createPinia()).mount("#app");
