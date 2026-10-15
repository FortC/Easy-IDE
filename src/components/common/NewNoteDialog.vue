<template>
  <transition name="emd-fade">
    <BaseModal
      v-if="ui.newNoteOpen"
      :title="t('nn.title')"
      width="sm"
      :show-close="false"
      @close="close"
    >
      <div class="nn-body">
        <div class="nn-row">
          <label>{{ t("nn.name") }}</label>
          <input
            ref="nameRef"
            v-model="name"
            type="text"
            :placeholder="t('nn.ph')"
            @keydown.enter="confirm"
          />
        </div>
        <div v-if="templates.length > 0" class="nn-row">
          <label>{{ t("nn.tpl") }}</label>
          <select v-model="template">
            <option value="">{{ t("nn.noTpl") }}</option>
            <option v-for="tpl in templates" :key="tpl.path" :value="tpl.path">
              {{ tpl.name }}
            </option>
          </select>
        </div>
      </div>
      <template #footer>
        <button class="emd-btn" @click="close">{{ t("c.cancel") }}</button>
        <button class="emd-btn emd-btn-accent" @click="confirm">{{ t("c.create") }}</button>
      </template>
    </BaseModal>
  </transition>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";
import BaseModal from "./BaseModal.vue";
import { useUiStore } from "../../stores/ui";
import { useVaultStore } from "../../stores/vault";
import { useSettingsStore } from "../../stores/settings";
import { api } from "../../ipc/tauri";
import { t } from "../../i18n";

const ui = useUiStore();
const vault = useVaultStore();
const settings = useSettingsStore();

const name = ref("");
const template = ref("");
const templates = ref<{ name: string; path: string }[]>([]);
const nameRef = ref<HTMLInputElement>();

onMounted(async () => {
  nextTick(() => nameRef.value?.focus());
  // 扫描模板文件夹
  const dir = (settings.data.templates_dir || "templates").replace(/^\/+|\/+$/g, "");
  try {
    const entries = await api.listDir(dir);
    templates.value = entries
      .filter((e) => !e.is_dir && e.kind === "md")
      .map((e) => ({ name: e.name.replace(/\.md$/i, ""), path: e.path }));
  } catch {
    templates.value = [];
  }
});

function applyTemplateVars(text: string, title: string): string {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return text
    .replace(/\{\{title\}\}/gi, title)
    .replace(/\{\{date\}\}/gi, `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`)
    .replace(/\{\{time\}\}/gi, `${pad(now.getHours())}:${pad(now.getMinutes())}`);
}

async function confirm() {
  const title = name.value.trim() || t("nn.untitled");
  let content = "";
  if (template.value) {
    try {
      content = await api.readTextFile(template.value);
      content = applyTemplateVars(content, title);
    } catch {
      content = "";
    }
  }
  close();
  await vault.newNote(ui.newNoteFolder, title, content);
}

function close() {
  ui.newNoteOpen = false;
}
</script>

<style scoped>
.nn-body {
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}
.nn-row {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.nn-row label {
  color: var(--text-muted);
  font-size: var(--font-ui-smaller);
}
</style>
