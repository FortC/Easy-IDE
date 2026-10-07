import { defineStore } from "pinia";
import { useSettingsStore } from "./settings";
import { t } from "../i18n";

/** 可配置的全局动作（id 与 App.vue 分发表对应） */
export interface KbAction {
  id: string;
  label: () => string;
  def: string;
}

export const KB_ACTIONS: KbAction[] = [
  { id: "fileSearch", label: () => t("sp.file"), def: "Ctrl+P" },
  { id: "classSearch", label: () => t("sp.class"), def: "Ctrl+N" },
  { id: "symbolSearch", label: () => t("sp.symbol"), def: "Ctrl+Shift+Alt+N" },
  { id: "contentSearch", label: () => t("sp.content"), def: "Ctrl+Shift+F" },
  { id: "newNote", label: () => t("sc.newNote"), def: "Ctrl+Alt+N" },
  { id: "daily", label: () => t("sc.daily"), def: "Ctrl+D" },
  { id: "cycleMode", label: () => t("sc.cycleMode"), def: "Ctrl+E" },
  { id: "graph", label: () => t("sc.graph"), def: "Ctrl+G" },
  { id: "settings", label: () => t("sc.settings"), def: "Ctrl+," },
  { id: "save", label: () => t("sc.save"), def: "Ctrl+S" },
  { id: "nextTab", label: () => t("sc.nextTab"), def: "Ctrl+Tab" },
  { id: "prevTab", label: () => t("sc.prevTab"), def: "Ctrl+Shift+Tab" },
  { id: "closeTab", label: () => t("tab.close"), def: "Ctrl+W" },
];

/** 键序列规范化："ctrl+shift+alt+n" 形式（小写、顺序固定） */
export function normalizeSeq(seq: string): string {
  const parts = seq
    .split("+")
    .map((p) => p.trim().toLowerCase())
    .filter(Boolean);
  const mods = ["ctrl", "shift", "alt"].filter((m) => parts.includes(m));
  const main = parts.filter((p) => !["ctrl", "shift", "alt"].includes(p));
  return [...mods, ...main].join("+");
}

/** 从键盘事件构造规范化序列 */
export function seqFromEvent(e: KeyboardEvent): string | null {
  const key = e.key;
  // 纯修饰键不构成序列
  if (["Control", "Shift", "Alt", "Meta"].includes(key)) return null;
  let main: string;
  if (key === "Tab") main = "tab";
  else if (key.length === 1) main = key.toLowerCase();
  else main = key.toLowerCase();
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push("ctrl");
  if (e.shiftKey) mods.push("shift");
  if (e.altKey) mods.push("alt");
  return [...mods, main].join("+");
}

export const useKeybindingsStore = defineStore("keybindings", {
  state: () => ({
    /** action id → 生效键序列（默认 + 用户覆盖合并） */
    bindings: {} as Record<string, string>,
  }),
  getters: {
    /** 反查表：序列 → action id（冲突时后注册者覆盖，UI 侧另有冲突标记） */
    reverse(): Record<string, string> {
      const out: Record<string, string> = {};
      for (const a of KB_ACTIONS) {
        const seq = this.bindings[a.id];
        if (seq) out[seq] = a.id;
      }
      return out;
    },
  },
  actions: {
    init() {
      const custom = useSettingsStore().data.keybindings ?? {};
      const merged: Record<string, string> = {};
      for (const a of KB_ACTIONS) {
        merged[a.id] = custom[a.id] ? normalizeSeq(custom[a.id]) : normalizeSeq(a.def);
      }
      this.bindings = merged;
    },
    async setBinding(id: string, seq: string) {
      const settings = useSettingsStore();
      settings.data.keybindings = { ...settings.data.keybindings, [id]: seq };
      await settings.persist();
      this.init();
    },
    async resetBinding(id: string) {
      const settings = useSettingsStore();
      if (settings.data.keybindings) delete settings.data.keybindings[id];
      await settings.persist();
      this.init();
    },
    async resetAll() {
      const settings = useSettingsStore();
      settings.data.keybindings = {};
      await settings.persist();
      this.init();
    },
    /** 冲突检测：同一序列被多个动作占用 */
    conflicts(): Record<string, true> {
      const seen = new Map<string, number>();
      for (const a of KB_ACTIONS) {
        const seq = this.bindings[a.id];
        if (seq) seen.set(seq, (seen.get(seq) ?? 0) + 1);
      }
      const out: Record<string, true> = {};
      for (const a of KB_ACTIONS) {
        const seq = this.bindings[a.id];
        if (seq && (seen.get(seq) ?? 0) > 1) out[a.id] = true;
      }
      return out;
    },
  },
});
