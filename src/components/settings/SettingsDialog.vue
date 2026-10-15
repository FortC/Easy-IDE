<template>
  <transition name="emd-fade">
    <div v-if="ui.settingsOpen" class="emd-modal-bg" @click.self="ui.settingsOpen = false">
    <div class="emd-modal st-modal">
      <div class="st-tabs">
        <BaseTabs v-model="tab" :items="tabItems" size="sm" aria-label="设置分类" />
      </div>
      <div class="st-body">
        <transition name="emd-pane" mode="out-in">
        <div v-if="tab === 'general'" key="general" class="st-pane">
          <div class="st-row">
            <label>{{ t("st.language") }}</label>
            <BaseSegmented
              :model-value="settings.data.language"
              :options="langOptions"
              :aria-label="t('st.language')"
              @update:model-value="settings.setLanguage($event as 'zh' | 'en')"
            />
          </div>
          <div class="st-row">
            <label>{{ t("st.fontSize") }}</label>
            <BaseSegmented
              :model-value="settings.data.font_scale"
              :options="fontScaleOptions"
              :aria-label="t('st.fontSize')"
              @update:model-value="settings.setFontScale($event as FontScaleKey)"
            />
          </div>
          <div class="st-row">
            <label>{{ t("st.theme") }}</label>
            <BaseSegmented
              :model-value="settings.data.theme"
              :options="themeOptions"
              :aria-label="t('st.theme')"
              @update:model-value="settings.setTheme($event as 'dark' | 'light')"
            />
          </div>
          <div class="st-row st-row-themes">
            <label>{{ t("st.codeTheme") }}</label>
            <div class="st-themes">
              <button
                v-for="th in codeThemeOptions"
                :key="th.key"
                class="st-theme-card"
                :class="{ 'is-on': settings.data.code_theme === th.key }"
                @click="settings.setCodeTheme(th.key)"
              >
                <span v-if="th.key === 'auto'" class="st-theme-sw">
                  <i class="st-theme-half" :style="{ background: autoPreview.dark }" />
                  <i class="st-theme-half" :style="{ background: autoPreview.light }" />
                </span>
                <span v-else class="st-theme-sw" :style="{ background: th.bg }">
                  <i v-for="(c, i) in th.dots" :key="i" class="st-theme-dot" :style="{ background: c }" />
                </span>
                <span class="st-theme-name">{{ t(th.label) }}</span>
              </button>
            </div>
          </div>
          <div class="st-row">
            <label>{{ t("st.motion") }}</label>
            <BaseSegmented
              :model-value="settings.data.motion_level"
              :options="motionOptions"
              :aria-label="t('st.motion')"
              @update:model-value="settings.setMotionLevel($event as MotionLevel)"
            />
          </div>
          <div class="st-row">
            <label>{{ t("st.attach") }}</label>
            <input v-model="settings.data.attachments_dir" type="text" @change="persist" />
          </div>
          <div class="st-row">
            <label>{{ t("st.tplDir") }}</label>
            <input v-model="settings.data.templates_dir" type="text" @change="persist" />
          </div>
          <div class="st-row">
            <label>{{ t("st.dailyDir") }}</label>
            <input v-model="settings.data.daily_dir" type="text" @change="persist" />
          </div>
          <div class="st-row">
            <label>{{ t("st.jotDir") }}</label>
            <input v-model="settings.data.jot_dir" type="text" @change="persist" />
          </div>
          <div class="st-row">
            <label>{{ t("st.dailyFmt") }}</label>
            <input v-model="settings.data.daily_format" type="text" @change="persist" />
          </div>
          <div class="st-row">
            <label>{{ t("st.dailyTpl") }}</label>
            <select v-model="settings.data.daily_template" @change="persist">
              <option :value="null">{{ t("st.noTpl") }}</option>
              <option v-for="t in dailyTemplates" :key="t.path" :value="t.name">{{ t.name }}</option>
            </select>
          </div>
          <div class="st-row">
            <label>{{ t("st.dailyTag") }}</label>
            <input
              v-model="settings.data.daily_tag"
              type="text"
              :placeholder="t('st.dailyTagPh')"
              @change="persist"
            />
          </div>
          <div class="st-row st-actions-row">
            <label>{{ t("st.index") }}</label>
            <button class="emd-btn" @click="rebuild">
              <Icon name="refresh-cw" :size="13" /> {{ t("st.rebuild") }}
            </button>
            <span v-if="rebuilt" class="st-ok">{{ tf("st.rebuilt", { n: count }) }}</span>
          </div>
          <div class="st-row st-actions-row">
            <label>{{ t("st.ctx") }}</label>
            <button class="emd-btn" @click="regMenu(true)">
              <Icon name="check" :size="13" /> {{ t('st.ctxReg') }}
            </button>
            <button class="emd-btn" @click="regMenu(false)">
              <Icon name="x" :size="13" /> {{ t("st.ctxUnreg") }}
            </button>
          </div>
          <p class="st-tip">
            {{ t("st.ctxTip") }}</p>
        </div>

        <div v-else-if="tab === 'appearance'" key="appearance" class="st-pane">
          <p class="st-tip">
            {{ t("st.snippetDir") }}<code>{{ settings.snippetsDir }}</code
            >{{ t("st.snippetTip2") }}
          </p>
          <div v-for="s in settings.data.css_snippets" :key="s.name" class="st-row">
            <label>{{ s.name }}</label>
            <label class="st-switch">
              <input
                type="checkbox"
                :checked="s.enabled"
                @change="toggleSnippet(s.name)"
              />
              {{ t("st.enable") }}
            </label>
            <button class="emd-btn" @click="removeSnippet(s.name)">{{ t("c.delete") }}</button>
          </div>
          <div class="st-row">
            <label>{{ t("st.addSnippet") }}</label>
            <button class="emd-btn" @click="addSnippet">
              <Icon name="plus" :size="13" /> {{ t("st.newSnippet") }}
            </button>
          </div>
        </div>

        <div v-else-if="tab === 'templates'" key="templates" class="st-pane">
          <TemplatePane />
        </div>

        <div v-else-if="tab === 'toolchain'" key="toolchain" class="st-pane">
          <ToolchainPane />
        </div>
        <div v-else-if="tab === 'shortcuts'" key="shortcuts" class="st-pane">
          <ShortcutsPane />
        </div>
        <div v-else-if="tab === 'ai'" key="ai" class="st-pane">
          <AiPane />
        </div>

        <div v-else-if="tab === 'mcp'" key="mcp" class="st-pane">
          <McpSettings />
        </div>

        <div v-else-if="tab === 'about'" key="about" class="st-pane">
          <AboutPane />
        </div>
        </transition>
      </div>
    </div>
  </div>
  </transition>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import Icon from "../common/Icon.vue";
import BaseSegmented from "../common/BaseSegmented.vue";
import BaseTabs from "../common/BaseTabs.vue";
import McpSettings from "./McpSettings.vue";
import TemplatePane from "./TemplatePane.vue";
import AiPane from "./AiPane.vue";
import ToolchainPane from "./ToolchainPane.vue";
import ShortcutsPane from "./ShortcutsPane.vue";
import AboutPane from "./AboutPane.vue";
import { useUiStore } from "../../stores/ui";
import { useSettingsStore } from "../../stores/settings";
import { useNotesIndexStore } from "../../stores/notesIndex";
import { api } from "../../ipc/tauri";
import { listTemplates, type TemplateFile } from "../../lib/template";
import { CODE_THEMES } from "../../lib/editorThemes";
import type { MotionLevel } from "../../types";
import { t, tf } from "../../i18n";

const ui = useUiStore();
const settings = useSettingsStore();
const indexStore = useNotesIndexStore();
const tab = ref("general");
const rebuilt = ref(false);
const count = ref(0);
const dailyTemplates = ref<TemplateFile[]>([]);

const fontScales = [
  { key: "xs", label: "st.fsXs" },
  { key: "sm", label: "st.fsSm" },
  { key: "md", label: "st.fsMd" },
  { key: "lg", label: "st.fsLg" },
  { key: "xl", label: "st.fsXl" },
];

/** 动效强度三档（完整 / 精简 / 关闭），与系统 prefers-reduced-motion 双保险 */
const motionLevels: { key: MotionLevel; label: string; hint: string }[] = [
  { key: "full", label: "st.motionFull", hint: "st.motionFullHint" },
  { key: "reduced", label: "st.motionReduced", hint: "st.motionReducedHint" },
  { key: "off", label: "st.motionOff", hint: "st.motionOffHint" },
];

// ---- BaseSegmented 选项（把 4 处手写 st-seg 收敛到统一基座）----
type FontScaleKey = (typeof fontScales)[number]["key"];

const langOptions = [
  { value: "zh", label: "简体中文" },
  { value: "en", label: "English (US)" },
];

const fontScaleOptions = computed(() =>
  fontScales.map((fs) => ({ value: fs.key, label: t(fs.label) })),
);

const themeOptions = computed(() => [
  { value: "dark", label: t("st.dark") },
  { value: "light", label: t("st.light") },
]);

const motionOptions = computed(() =>
  motionLevels.map((m) => ({ value: m.key, label: t(m.label), title: t(m.hint) })),
);

/** 代码配色主题选项：跟随界面 + 内置 5 套（色值见 lib/editorThemes.ts） */
const codeThemeOptions = computed(() => [
  { key: "auto", label: "ct.auto", bg: "", dots: [] as string[] },
  ...CODE_THEMES.map((th) => ({ key: th.key, label: th.label(), bg: th.swatch[0], dots: th.swatch.slice(1) })),
]);

/** 「跟随界面」卡片的预览：深/浅两半，取自对应主题底色 */
const autoPreview = computed(() => ({
  dark: CODE_THEMES.find((th) => th.key === "deep")?.colors.bg ?? "#1e1e1e",
  light: CODE_THEMES.find((th) => th.key === "paper")?.colors.bg ?? "#ffffff",
}));

// computed：切换语言时页签文案同步更新（结构即 BaseTabs 的 items）
const tabItems = computed(() => [
  { key: "general", label: t("st.general") },
  { key: "templates", label: t("st.templates") },
  { key: "ai", label: t("st.ai") },
  { key: "toolchain", label: t("st.toolchain") },
  { key: "shortcuts", label: t("st.shortcuts") },
  { key: "appearance", label: t("st.appearance") },
  { key: "mcp", label: t("st.mcp") },
  { key: "about", label: t("st.about") },
]);

onMounted(async () => {
  dailyTemplates.value = await listTemplates();
});

async function persist() {
  await settings.persist();
}

async function rebuild() {
  count.value = (await indexStore.rebuild()).length;
  rebuilt.value = true;
  setTimeout(() => (rebuilt.value = false), 4000);
}

async function setDefaultApp() {
  try {
    await api.openDefaultAppsSettings();
  } catch (e) {
    alert(String(e));
  }
}

async function regMenu(register: boolean) {
  try {
    if (register) await api.registerContextMenu();
    else await api.unregisterContextMenu();
    alert(register ? t("st.ctxOk") : t("st.ctxRemoved"));
  } catch (e) {
    alert(`${t("st.opFail")}: ${e}`);
  }
}

async function toggleSnippet(name: string) {
  const s = settings.data.css_snippets.find((x) => x.name === name);
  if (s) s.enabled = !s.enabled;
  await settings.persist();
  window.dispatchEvent(new CustomEvent("emd-snippets-changed"));
}

async function removeSnippet(name: string) {
  await api.deleteSnippetFile(name);
  settings.data.css_snippets = settings.data.css_snippets.filter(
    (x) => x.name !== name,
  );
  await settings.persist();
  window.dispatchEvent(new CustomEvent("emd-snippets-changed"));
}

async function addSnippet() {
  const name = prompt(t("st.snippetName"));
  if (!name) return;
  try {
    await api.createSnippetFile(name.trim());
  } catch (e) {
    alert(String(e));
    return;
  }
  settings.data.css_snippets.push({ name: name.trim() + ".css", enabled: true });
  await settings.persist();
  window.dispatchEvent(new CustomEvent("emd-snippets-changed"));
}
</script>

<style scoped>
.st-modal {
  width: 640px;
  height: 480px;
}
.st-tabs {
  display: flex;
  gap: var(--space-1);
  padding: 10px 12px 0;
  border-bottom: 1px solid var(--background-modifier-border);
}
/* 页签与分段控件本体已由 BaseTabs / BaseSegmented 提供，此处只管容器 */
.st-body {
  flex: 1;
  overflow-y: auto;
}
.st-pane {
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.st-row {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--background-modifier-border);
}
.st-row label:first-child {
  width: 190px;
  flex-shrink: 0;
  color: var(--text-muted);
}
.st-row input[type="text"] {
  flex: 1;
}
.st-row select {
  flex: 1;
}
/* 代码配色主题选择卡片 */
.st-row-themes {
  align-items: flex-start;
}
.st-themes {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.st-theme-card {
  width: 104px;
  padding: 6px;
  border-radius: var(--radius-m);
  border: 2px solid var(--background-modifier-border);
  background: var(--background-primary);
  display: flex;
  flex-direction: column;
  gap: 5px;
  transition: border-color var(--anim-hover), transform var(--anim-press);
}
.st-theme-card:hover {
  border-color: var(--background-modifier-border-hover);
}
.st-theme-card:active {
  transform: scale(0.96);
}
.st-theme-card.is-on {
  border-color: var(--interactive-accent);
}
.st-theme-sw {
  height: 32px;
  border-radius: var(--radius-s);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  overflow: hidden;
  border: 1px solid var(--background-modifier-border);
}
.st-theme-half {
  flex: 1;
  height: 100%;
}
.st-theme-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.st-theme-name {
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  white-space: nowrap;
}
.st-theme-card.is-on .st-theme-name {
  color: var(--text-normal);
  font-weight: 600;
}
.st-ok {
  color: var(--text-success);
  font-size: var(--font-ui-smaller);
}
.st-tip {
  color: var(--text-muted);
  padding: 6px 0;
}
.st-tip code {
  background: var(--code-background);
  border-radius: var(--radius-s);
  padding: 2px 6px;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  user-select: text;
}
.st-switch {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-muted);
}
</style>
