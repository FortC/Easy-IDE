<template>
  <div class="tc-pane">
    <div class="st-row tc-desc">{{ t("tc.desc") }}</div>
    <div class="st-row" v-for="row in rows" :key="row.kind">
      <label>{{ row.label }}</label>
      <div class="tc-main">
        <input v-model="row.path" type="text" :placeholder="row.placeholder" @change="persist" />
        <div class="tc-status">
          <template v-if="row.info">
            <Icon v-if="row.info.found" name="check" :size="12" class="tc-ok" />
            <Icon v-else name="alert-triangle" :size="12" class="tc-bad" />
            <span :class="row.info.found ? 'tc-ok' : 'tc-bad'">
              {{ row.info.found ? row.info.version || t("tc.found") : row.info.error || t("tc.missing") }}
            </span>
          </template>
          <span v-else class="tc-hint">{{ t("tc.hint") }}</span>
        </div>
      </div>
      <button class="emd-btn tc-detect" :title="t('tc.detect')" @click="detect(row)">
        {{ t("tc.detect") }}
      </button>
    </div>
    <div class="st-row tc-actions">
      <button class="emd-btn emd-btn-accent" :disabled="detecting" @click="detectAll">
        <Icon name="refresh-cw" :size="12" :class="{ 'is-spin': detecting }" />
        {{ t("tc.detectAll") }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from "vue";
import Icon from "../common/Icon.vue";
import { useSettingsStore } from "../../stores/settings";
import { api } from "../../ipc/tauri";
import type { ToolInfo } from "../../types";
import { t } from "../../i18n";

const settings = useSettingsStore();
const detecting = ref(false);

const rows = reactive([
  { kind: "jdk", label: "JDK", get: () => settings.data.jdk_path, set: (v: string) => (settings.data.jdk_path = v), placeholder: "C:\\Program Files\\Java\\jdk-17（留空自动探测）", path: "", info: null as ToolInfo | null },
  { kind: "maven", label: "Maven", get: () => settings.data.maven_path, set: (v: string) => (settings.data.maven_path = v), placeholder: "C:\\apache-maven-3.9.x（留空自动探测）", path: "", info: null as ToolInfo | null },
  { kind: "node", label: "Node", get: () => settings.data.node_path, set: (v: string) => (settings.data.node_path = v), placeholder: "node.exe 路径（留空自动探测）", path: "", info: null as ToolInfo | null },
  { kind: "git", label: "Git", get: () => settings.data.git_path, set: (v: string) => (settings.data.git_path = v), placeholder: "git.exe 路径（留空自动探测）", path: "", info: null as ToolInfo | null },
]);
for (const r of rows) r.path = r.get();

async function persist() {
  for (const r of rows) r.set(r.path.trim());
  await settings.persist();
}

async function detect(row: (typeof rows)[number]) {
  await persist();
  row.info = await api.toolchainDetect(row.kind, row.path.trim() || null);
}

async function detectAll() {
  detecting.value = true;
  try {
    await persist();
    for (const row of rows) {
      row.info = await api.toolchainDetect(row.kind, row.path.trim() || null);
    }
  } finally {
    detecting.value = false;
  }
}
</script>

<style scoped>
.tc-pane {
  display: flex;
  flex-direction: column;
}
.tc-pane .st-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px 0;
  border-bottom: 1px solid var(--background-modifier-border);
}
.tc-pane .st-row > label {
  width: 60px;
  flex-shrink: 0;
  color: var(--text-muted);
  padding-top: 5px;
}
.tc-desc {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  line-height: 1.6;
  border-bottom: none;
}
.tc-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.tc-status {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: var(--font-ui-smaller);
  min-height: 16px;
}
.tc-ok {
  color: #4ec970;
}
.tc-bad {
  color: var(--text-error, #e93147);
}
.tc-hint {
  color: var(--text-faint);
}
.tc-detect {
  flex-shrink: 0;
}
.tc-actions {
  margin-top: 8px;
}
.is-spin {
  animation: tc-spin 1s linear infinite;
}
@keyframes tc-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
