import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 推荐配置：固定端口、失败即退出、不打印杂音
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "chrome110",
    minify: "esbuild",
    sourcemap: false,
    rollupOptions: {
      output: {
        // 显式分包：编辑器内核与框架层单独成块，便于长期缓存复用。
        // 用函数式而非对象式 —— 对象式要求包有根导出，
        // 而 @codemirror/legacy-modes 只有子路径导出（@codemirror/legacy-modes/mode/xxx）。
        manualChunks(id) {
          if (!id.includes("node_modules")) return undefined;
          if (id.includes("@codemirror") || id.includes("@lezer")) {
            return "vendor-codemirror";
          }
          if (id.includes("markdown-it") || id.includes("/yaml/")) {
            return "vendor-markdown";
          }
          if (id.includes("/vue/") || id.includes("@vue") || id.includes("pinia")) {
            return "vendor-vue";
          }
          return undefined;
        },
      },
    },
  },
});
