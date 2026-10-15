<template>
  <div class="mm-panel">
    <div class="emd-panel-header">
      <span>{{ t("mm.title") }}</span>
      <button class="mm-add" :title="t('mm.add')" @click="startAdd">
        <Icon name="plus" :size="13" />
      </button>
    </div>

    <!-- 新增记忆 -->
    <div v-if="drafting" class="mm-form">
      <input
        v-model="draftTitle"
        class="mm-input"
        :placeholder="t('mm.titlePh')"
        @keydown.enter="saveDraft"
        @keydown.esc="cancelDraft"
      />
      <textarea
        v-model="draftBody"
        class="mm-textarea"
        :placeholder="t('mm.bodyPh')"
        rows="4"
        @keydown.esc="cancelDraft"
      />
      <div class="mm-actions">
        <button class="mm-btn mm-btn-cancel" @click="cancelDraft">{{ t("c.cancel") }}</button>
        <button class="mm-btn mm-btn-primary" :disabled="!draftTitle.trim()" @click="saveDraft">
          {{ t("c.confirm") }}
        </button>
      </div>
    </div>

    <!-- 记忆列表（AI 自动积累 + 手动添加） -->
    <div v-if="memories.length === 0 && !drafting" class="mm-empty">
      {{ t("mm.empty") }}
      <span class="mm-empty-hint">{{ t("mm.emptyHint") }}</span>
    </div>
    <div v-else class="mm-scroll">
      <div
        v-for="m in memories"
        :key="m.title + m.body"
        class="mm-card"
        :title="t('mm.injected')"
      >
        <div class="mm-card-head">
          <span class="mm-card-title">{{ m.title }}</span>
          <button class="mm-del" :title="t('mm.del')" @click="remove(m)">
            <Icon name="x" :size="10" />
          </button>
        </div>
        <div class="mm-card-body">{{ m.body }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import { api } from "../../ipc/tauri";
import { useVaultStore } from "../../stores/vault";
import { t } from "../../i18n";

/** 一条项目级记忆（AI 会话与构建过程中的沉淀） */
interface Memory {
  title: string;
  body: string;
}

const vault = useVaultStore();

const memories = ref<Memory[]>([]);
const drafting = ref(false);
const draftTitle = ref("");
const draftBody = ref("");

/** 记忆存项目级 .easyide/ 目录，随项目走、不污染用户全局 */
const MEM_FILE = ".easyide/memories.json";

async function load() {
  try {
    const raw = await api.readTextFile(MEM_FILE);
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) memories.value = parsed;
  } catch {
    /* 文件不存在时静默，等同于空记忆 */
  }
}

async function persist() {
  try {
    await api.writeTextFile(MEM_FILE, JSON.stringify(memories.value, null, 2));
  } catch (e) {
    console.error("保存 AI 记忆失败：", e);
  }
}

function startAdd() {
  drafting.value = true;
  draftTitle.value = "";
  draftBody.value = "";
}

function cancelDraft() {
  drafting.value = false;
  draftTitle.value = "";
  draftBody.value = "";
}

async function saveDraft() {
  const title = draftTitle.value.trim();
  if (!title) return;
  memories.value.unshift({ title, body: draftBody.value.trim() });
  cancelDraft();
  await persist();
}

async function remove(m: Memory) {
  memories.value = memories.value.filter((x) => x !== m);
  await persist();
}

watch(() => vault.root, load, { immediate: true });
</script>

<style scoped>
.mm-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.mm-add {
  margin-left: auto;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.mm-add:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.mm-form {
  padding: 8px;
  border-bottom: 1px solid var(--background-modifier-border);
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
}
.mm-input,
.mm-textarea {
  width: 100%;
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
  padding: 4px 6px;
  color: var(--text-normal);
  font-family: var(--font-ui);
  font-size: var(--font-ui-size);
  transition: border-color var(--anim-hover);
}
.mm-textarea {
  resize: vertical;
  line-height: 1.5;
}
.mm-input:focus,
.mm-textarea:focus {
  outline: none;
  border-color: var(--interactive-accent);
}
.mm-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}
.mm-btn {
  padding: 3px 12px;
  border-radius: var(--radius-s);
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  transition: background var(--anim-hover), color var(--anim-hover),
    transform var(--anim-press);
}
.mm-btn-cancel:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.mm-btn-primary {
  background: var(--interactive-accent);
  color: var(--text-on-accent);
}
.mm-btn-primary:disabled {
  opacity: 0.45;
}
.mm-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 6px 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.mm-card {
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  padding: 6px 8px;
  transition: border-color var(--anim-hover);
}
.mm-card:hover {
  border-color: var(--background-modifier-border-hover);
}
.mm-card-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 3px;
}
.mm-card-title {
  flex: 1;
  font-size: var(--font-ui-size);
  font-weight: 500;
  color: var(--text-normal);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mm-del {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-faint);
  opacity: 0;
  transition: opacity var(--anim-hover), color var(--anim-hover);
}
.mm-card:hover .mm-del {
  opacity: 1;
}
.mm-del:hover {
  color: var(--text-error);
}
.mm-card-body {
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
}
.mm-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 16px;
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  text-align: center;
}
.mm-empty-hint {
  opacity: 0.75;
}
</style>
