import { defineStore } from "pinia";
import { api } from "../ipc/tauri";
import { defaultSettings, type AppSettings, type MotionLevel } from "../types";
import { setLang } from "../i18n";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    loaded: false,
    data: defaultSettings(),
    configDir: "",
    snippetsDir: "",
  }),
  actions: {
    async init() {
      if (this.loaded) return;
      try {
        this.data = { ...defaultSettings(), ...(await api.getSettings()) };
      } catch {
        this.data = defaultSettings();
      }
      try {
        const dirs = await api.getConfigDirs();
        this.configDir = dirs.config_dir;
        this.snippetsDir = dirs.snippets_dir;
      } catch {
        /* 忽略 */
      }
      this.loaded = true;
      this.applyTheme();
      setLang(this.data.language === "en" ? "en" : "zh");
      this.applyFontScale();
      this.applyMotionLevel();
    },
    async persist() {
      await api.saveSettings(this.data);
    },
    setTheme(theme: "dark" | "light") {
      this.data.theme = theme;
      this.applyTheme(true);
      this.persist();
    },
    /** 代码配色主题（"auto" 跟随界面；SourceEditor 监听字段变化即时换肤） */
    setCodeTheme(key: string) {
      this.data.code_theme = key;
      this.persist();
    },
    toggleTheme() {
      this.setTheme(this.data.theme === "dark" ? "light" : "dark");
    },
    setLanguage(lang: "zh" | "en") {
      this.data.language = lang;
      setLang(lang);
      this.persist();
    },
    applyFontScale() {
      const map: Record<string, number> = { xs: 0.85, sm: 0.925, md: 1, lg: 1.1, xl: 1.2 };
      const scale = map[this.data.font_scale] || 0.925;
      document.documentElement.style.setProperty("--font-scale", String(scale));
    },
    setFontScale(scale: string) {
      this.data.font_scale = scale;
      this.applyFontScale();
      this.persist();
    },
    /** 动效强度：挂 data-motion 到 <html>，与系统 prefers-reduced-motion 双保险 */
    applyMotionLevel() {
      const lv = this.data.motion_level === "off" || this.data.motion_level === "reduced"
        ? this.data.motion_level
        : "full";
      document.documentElement.setAttribute("data-motion", lv);
    },
    setMotionLevel(level: MotionLevel) {
      this.data.motion_level = level;
      this.applyMotionLevel();
      this.persist();
    },
    applyTheme(animate = false) {
      const apply = () => {
        document.documentElement.dataset.theme =
          this.data.theme === "light" ? "light" : "dark";
      };
      // 主题切换用 View Transitions 做一次合成器快照 crossfade（时长见 theme.css）；
      // 不支持该 API 或系统要求减少动效时直接切换，零额外开销
      const doc = document as unknown as { startViewTransition?: (cb: () => void) => void };
      if (
        animate &&
        doc.startViewTransition &&
        !window.matchMedia("(prefers-reduced-motion: reduce)").matches
      ) {
        doc.startViewTransition(apply);
      } else {
        apply();
      }
    },
  },
});
