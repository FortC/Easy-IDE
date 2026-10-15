<template>
  <div class="ai-pane">
    <div class="st-row">
      <label>{{ t("ai.protocol") }}</label>
      <div class="ai-seg">
        <button :class="{ 'is-on': settings.data.ai_provider === 'openai' }" @click="setProvider('openai')">
          {{ t("ai.openai") }}
        </button>
        <button :class="{ 'is-on': settings.data.ai_provider === 'anthropic' }" @click="setProvider('anthropic')">
          Anthropic
        </button>
      </div>
    </div>
    <div class="st-row">
      <label>{{ t("ai.base") }}</label>
      <input v-model="settings.data.ai_base_url" type="text" placeholder="https://api.openai.com/v1" @change="persist" />
    </div>
    <div class="st-row">
      <label>{{ t("ai.key") }}</label>
      <input v-model="settings.data.ai_api_key" type="password" placeholder="sk-…" @change="persist" />
    </div>
    <div class="st-row">
      <label>{{ t("ai.model") }}</label>
      <input v-model="settings.data.ai_model" type="text" :placeholder="modelPlaceholder" @change="persist" />
    </div>
    <div class="st-row st-actions-row">
      <label>{{ t("ai.conn") }}</label>
      <button class="emd-btn" :disabled="testing" @click="test">
        <Icon name="play" :size="12" /> {{ testing ? t("ai.testing") : t("ai.test") }}
      </button>
      <span v-if="testOk" class="ai-ok">{{ t("ai.ok") }}</span>
    </div>
    <p class="st-tip">
      {{ t("ai.tip2") }}
    </p>

    <!-- B7-B 档：本地模型端点（Ollama / llama.cpp 等 OpenAI 兼容服务） -->
    <div class="ai-local">
      <div class="ai-local-head">
        <span class="ai-local-title">{{ t("ai.localTitle") }}</span>
        <label class="ai-local-switch">
          <input v-model="settings.data.local_llm_enabled" type="checkbox" @change="onLocalToggle" />
          {{ settings.data.local_llm_enabled ? t("ai.localOn") : t("ai.localOff") }}
        </label>
      </div>
      <p class="ai-local-tip">{{ t("ai.localTip") }}</p>
      <div class="st-row">
        <label>{{ t("ai.localBase") }}</label>
        <input
          v-model="settings.data.local_llm_base"
          type="text"
          placeholder="http://localhost:11434/v1"
          @change="persist"
        />
      </div>
      <div class="st-row">
        <label>{{ t("ai.localModel") }}</label>
        <input
          v-model="settings.data.local_llm_model"
          type="text"
          :placeholder="t('ai.localModelPh')"
          list="ai-local-models"
          @change="persist"
        />
        <datalist id="ai-local-models">
          <option v-for="m in localModels" :key="m" :value="m" />
        </datalist>
      </div>
      <div class="st-row st-actions-row">
        <label>{{ t("ai.localTest") }}</label>
        <button class="emd-btn" :disabled="localTesting" @click="testLocal">
          <Icon name="play" :size="12" /> {{ localTesting ? t("ai.testing") : t("ai.test") }}
        </button>
        <span v-if="localOk" class="ai-ok">{{ t("ai.localOk") }}（{{ localElapsed }} ms）</span>
        <span v-else-if="localErr" class="ai-local-err">{{ localErr }}</span>
      </div>
      <div v-if="localModels.length" class="ai-local-list">
        <span class="ai-local-list-t">{{ tf("ai.localFound", { n: localModels.length }) }}</span>
        <button
          v-for="m in localModels"
          :key="m"
          class="ai-local-chip"
          :class="{ 'is-on': settings.data.local_llm_model === m }"
          @click="pickLocalModel(m)"
        >
          {{ m }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import Icon from "../common/Icon.vue";
import { useSettingsStore } from "../../stores/settings";
import { api } from "../../ipc/tauri";
import { t, tf } from "../../i18n";

const settings = useSettingsStore();
const testing = ref(false);
const testOk = ref(false);

const modelPlaceholder = computed(() =>
  settings.data.ai_provider === "anthropic" ? "claude-sonnet-4-20250514" : "gpt-4o-mini",
);

function setProvider(p: "openai" | "anthropic") {
  settings.data.ai_provider = p;
  settings.data.ai_base_url =
    p === "anthropic" ? "https://api.anthropic.com" : "https://api.openai.com/v1";
  settings.data.ai_model = p === "anthropic" ? "claude-sonnet-4-20250514" : "gpt-4o-mini";
  testOk.value = false;
  persist();
}

async function persist() {
  await settings.persist();
}

async function test() {
  testing.value = true;
  testOk.value = false;
  try {
    const r = await api.aiChat("请只回复两个字：正常", "这是连通性测试。");
    testOk.value = r.trim().length > 0;
    alert(`${t("ai.reply")}${r.slice(0, 100)}`);
  } catch (e) {
    alert(`${t("ai.testFail")}: ${e}`);
  } finally {
    testing.value = false;
  }
}

// ---- B7-B 档：本地模型端点 ----
const localTesting = ref(false);
const localOk = ref(false);
const localErr = ref("");
const localElapsed = ref(0);
const localModels = ref<string[]>([]);

async function onLocalToggle() {
  localOk.value = false;
  localErr.value = "";
  await persist();
  // 开启即自动探测一次：让用户立刻知道服务起没起，而不是还要再点一次按钮
  if (settings.data.local_llm_enabled) await testLocal();
}

async function testLocal() {
  localTesting.value = true;
  localOk.value = false;
  localErr.value = "";
  try {
    const r = await api.localLlmProbe(settings.data.local_llm_base || null);
    localElapsed.value = r.elapsed_ms;
    localModels.value = r.models;
    if (r.reachable) {
      localOk.value = true;
      // 探测到了模型但当前填的模型不在列表里，多半是填错了，直接提示比让用户猜好
      const cur = settings.data.local_llm_model.trim();
      if (cur && r.models.length && !r.models.includes(cur)) {
        localErr.value = `当前模型 ${cur} 不在端点可用列表中`;
        localOk.value = false;
      } else if (!cur && r.models.length) {
        await pickLocalModel(r.models[0]);
      }
    } else {
      localErr.value = r.error;
    }
  } catch (e) {
    localErr.value = String(e);
  } finally {
    localTesting.value = false;
  }
}

async function pickLocalModel(m: string) {
  settings.data.local_llm_model = m;
  localOk.value = false;
  localErr.value = "";
  await persist();
}
</script>

<style scoped>
.ai-pane {
  display: flex;
  flex-direction: column;
}
.ai-pane .st-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
  border-bottom: 1px solid var(--background-modifier-border);
}
.ai-pane .st-row label {
  width: 90px;
  flex-shrink: 0;
  color: var(--text-muted);
}
.ai-pane input {
  flex: 1;
}
.ai-seg {
  display: flex;
  background: var(--interactive-normal);
  border-radius: var(--radius-s);
  padding: 2px;
}
.ai-seg button {
  padding: 4px 14px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.ai-seg button.is-on {
  background: var(--background-primary);
  color: var(--text-normal);
}
.ai-ok {
  color: var(--text-success);
  font-size: var(--font-ui-smaller);
}
.st-tip {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  line-height: 1.6;
  padding: 10px 0 0;
}
/* ---- B7-B 档：本地模型分组 ---- */
.ai-local {
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid var(--background-modifier-border);
}
.ai-local-head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.ai-local-title {
  font-size: var(--font-ui-size);
  font-weight: 600;
  color: var(--text-normal);
}
.ai-local-switch {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  cursor: pointer;
}
.ai-local-tip {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  line-height: 1.6;
  padding: 6px 0 2px;
}
.ai-local-err {
  color: var(--text-error);
  font-size: var(--font-ui-smaller);
  word-break: break-all;
}
.ai-local-list {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  padding-top: 8px;
}
.ai-local-list-t {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.ai-local-chip {
  font-size: var(--font-ui-smaller);
  padding: 1px 8px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  background: var(--background-modifier-hover);
}
.ai-local-chip.is-on {
  color: var(--text-success);
  background: var(--background-modifier-hover);
  box-shadow: inset 0 -1px 0 currentColor;
}
</style>
