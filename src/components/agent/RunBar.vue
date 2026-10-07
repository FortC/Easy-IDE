<template>
  <div class="runbar">
    <div class="rb-group">
      <button
        v-for="p in presets"
        :key="p.key"
        class="rb-preset"
        :title="p.title"
        :disabled="agent.streaming"
        @click="runPreset(p.key)"
      >
        <Icon :name="p.icon" :size="13" />
        <span>{{ p.label }}</span>
      </button>
    </div>
    <div class="rb-right">
      <button class="rb-preset rb-pkg" :title="t('pk.title')" @click="ui.pkgOpen = true">
        <Icon name="archive" :size="13" />
        <span>{{ t("pk.short") }}</span>
      </button>
      <button
        class="rb-console-btn"
        :class="{ 'has-running': consoleStore.running.length > 0 }"
        :title="t('cs.title')"
        @click="ui.consoleVisible = !ui.consoleVisible"
      >
        <Icon name="terminal" :size="13" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import Icon from "../common/Icon.vue";
import { useAgentStore } from "../../stores/agent";
import { useConsoleStore } from "../../stores/console";
import { useEditorStore } from "../../stores/editor";
import { useUiStore } from "../../stores/ui";
import { t } from "../../i18n";

const agent = useAgentStore();
const consoleStore = useConsoleStore();
const editor = useEditorStore();
const ui = useUiStore();

type PresetKey = "run" | "debug" | "build" | "deps" | "fix" | "explain" | "test";

const presets = computed(() => [
  { key: "run" as PresetKey, icon: "play", label: t("rb2.run"), title: t("rb2.runTitle") },
  { key: "debug" as PresetKey, icon: "bug", label: t("rb2.debug"), title: t("rb2.debugTitle") },
  { key: "build" as PresetKey, icon: "hammer", label: t("rb2.build"), title: t("rb2.buildTitle") },
  { key: "deps" as PresetKey, icon: "package", label: t("rb2.deps"), title: t("rb2.depsTitle") },
  { key: "fix" as PresetKey, icon: "wrench", label: t("rb2.fix"), title: t("rb2.fixTitle") },
  { key: "explain" as PresetKey, icon: "book", label: t("rb2.explain"), title: t("rb2.explainTitle") },
  { key: "test" as PresetKey, icon: "check-square", label: t("rb2.test"), title: t("rb2.testTitle") },
]);

function presetPrompt(key: PresetKey): string {
  switch (key) {
    case "run":
      return "请运行当前项目：先用工具查看项目根目录与构建文件（pom.xml / build.gradle / package.json 等）判断项目类型与入口，确定运行命令后用 run_command 执行（会请求用户确认），执行后把关键输出（启动成功标志 / 端口 / 报错）汇报给我。";
    case "debug":
      return "请以调试模式运行当前项目：先判断项目类型，选择带调试参数的运行方式（如 JVM 加 -Ddebug、Spring Boot 加 --debug，或打印详细日志的配置），用 run_command 执行（会请求用户确认），然后重点汇总日志里的 ERROR 与 WARN 及其原因。若控制台已有运行中的进程请先说明。";
    case "build":
      return "请构建当前项目（跳过测试，如 mvn package -DskipTests 或 npm run build）：先确认构建方式，用 run_command 执行（会请求用户确认），结束后汇报构建结果与产物路径。";
    case "deps":
      return "请拉取/刷新当前项目依赖（如 mvn dependency:resolve 或 npm install）：先确认包管理方式，用 run_command 执行（会请求用户确认），结束后汇报结果（新增/变更的依赖或失败原因）。";
    case "fix": {
      const tail = consoleStore.tail;
      return `请帮我修复最近的报错。先看下面的控制台输出定位原因，需要时用工具（read_file / search_symbols / search_text）查看相关代码，然后给出具体修改方案（文件、位置、改法）；如果修改很小且明确，说明改完后我可以手动应用。\n\n最近控制台输出：\n${tail || "（暂无输出，请先运行一次项目）"}`;
    }
    case "explain":
      return "请解释当前项目的结构：用工具浏览根目录、关键构建文件与源码目录，给出模块划分、技术栈（框架与版本）、配置要点与启动方式的简明项目地图。";
    case "test": {
      const file = editor.activePath || "（未打开文件）";
      return `请为当前打开的文件生成单元测试骨架（只生成代码，不执行）：先用 read_file 查看该文件，再给出完整的测试类代码与依赖说明。当前文件：${file}`;
    }
  }
}

function runPreset(key: PresetKey) {
  const prompt = presetPrompt(key);
  if (!prompt) return;
  agent.openPanel();
  void agent.run(prompt);
}
</script>

<style scoped>
.runbar {
  height: 34px;
  flex-shrink: 0;
  display: flex;
  align-items: stretch;
  gap: 8px;
  padding: 0 8px;
  background: var(--background-secondary);
  border-bottom: 1px solid var(--background-modifier-border);
}
.rb-group {
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
  scrollbar-width: none;
}
.rb-group::-webkit-scrollbar {
  display: none;
}
.rb-preset {
  height: 24px;
  padding: 0 9px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  font-size: var(--font-ui-size);
  white-space: nowrap;
  flex-shrink: 0;
  transition: background var(--anim-fast), color var(--anim-fast);
}
.rb-preset:hover:not(:disabled) {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.rb-preset:disabled {
  opacity: 0.5;
}
.rb-preset:first-child {
  color: var(--text-on-accent);
  background: var(--interactive-accent);
}
.rb-preset:first-child:hover:not(:disabled) {
  background: var(--interactive-accent-hover);
  color: var(--text-on-accent);
}
.rb-right {
  margin-left: auto;
  display: flex;
  align-items: center;
}
.rb-console-btn {
  width: 26px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.rb-console-btn:hover,
.rb-console-btn.has-running {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.rb-console-btn.has-running {
  color: var(--interactive-accent);
}
</style>
