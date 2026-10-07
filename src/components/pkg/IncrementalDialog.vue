<template>
  <transition name="emd-fade">
    <div v-if="ui.pkgOpen" class="emd-modal-bg" @click.self="ui.pkgOpen = false">
      <div class="emd-modal pk-modal">
        <div class="pk-title">{{ t("pk.title") }}</div>

        <div class="pk-row">
          <label>{{ t("pk.baseline") }}</label>
          <select v-model="baseline" class="pk-select">
            <option value="" disabled>{{ t("pk.pickBaseline") }}</option>
            <optgroup v-if="baselines.tags.length" :label="t('pk.tags')">
              <option v-for="tag in baselines.tags" :key="tag" :value="tag">{{ tag }}</option>
            </optgroup>
            <optgroup :label="t('pk.commits')">
              <option v-for="c in baselines.commits" :key="c.hash" :value="c.hash">
                {{ c.hash }} {{ c.date }} {{ c.subject }}
              </option>
            </optgroup>
          </select>
        </div>

        <div class="pk-row">
          <label>{{ t("pk.template") }}</label>
          <div class="pk-seg">
            <button
              v-for="tp in templates"
              :key="tp.key"
              class="pk-seg-btn"
              :class="{ 'is-active': template === tp.key }"
              @click="template = tp.key"
            >
              {{ tp.label }}
            </button>
          </div>
        </div>
        <div class="pk-tip">{{ templateTip }}</div>

        <div class="pk-actions">
          <button
            class="emd-btn emd-btn-accent"
            :disabled="!baseline || building"
            @click="build"
          >
            <Icon name="archive" :size="13" />
            {{ building ? t("pk.building") : t("pk.build") }}
          </button>
        </div>

        <!-- 构建结果 -->
        <div v-if="result" class="pk-result">
          <div class="pk-result-head">
            <Icon name="check" :size="13" />
            <span class="pk-zip">{{ result.zip_path }}</span>
            <button class="emd-btn pk-copy" @click="copyPath">{{ t("c.copy") }}</button>
          </div>
          <div class="pk-entries">
            <div v-for="e in result.entries.slice(0, 50)" :key="e" class="pk-entry">{{ e }}</div>
            <div v-if="result.entries.length > 50" class="pk-entry-more">
              … {{ result.entries.length - 50 }} more
            </div>
          </div>
          <div v-if="result.skipped.length" class="pk-skipped">
            <div class="pk-skipped-title">{{ t("pk.skipped") }}</div>
            <div v-for="s in result.skipped.slice(0, 10)" :key="s" class="pk-entry">{{ s }}</div>
          </div>
        </div>
        <div v-if="buildError" class="pk-error">{{ buildError }}</div>

        <!-- 历史 -->
        <div v-if="history.length" class="pk-history">
          <div class="pk-history-title">{{ t("pk.history") }}</div>
          <div v-for="h in history.slice(0, 8)" :key="h.time + h.zip" class="pk-history-item">
            <span class="pk-h-time">{{ h.time }}</span>
            <span class="pk-h-base">{{ h.baseline }}</span>
            <span class="pk-h-tpl">{{ h.template }}</span>
            <span class="pk-h-count">{{ h.count }} 项</span>
            <span class="pk-h-zip">{{ fileName(h.zip) }}</span>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import { useUiStore } from "../../stores/ui";
import { api } from "../../ipc/tauri";
import type { PkgBaselines, PkgHistoryItem, PkgResult } from "../../types";
import { t } from "../../i18n";

const ui = useUiStore();
const baselines = ref<PkgBaselines>({ tags: [], commits: [] });
const history = ref<PkgHistoryItem[]>([]);
const baseline = ref("");
const template = ref("classes");
const building = ref(false);
const result = ref<PkgResult | null>(null);
const buildError = ref("");

const templates = computed(() => [
  { key: "classes", label: t("pk.tplClasses") },
  { key: "war", label: t("pk.tplWar") },
  { key: "fatjar", label: t("pk.tplFatjar") },
]);

const templateTip = computed(() =>
  template.value === "war"
    ? t("pk.tipWar")
    : template.value === "fatjar"
      ? t("pk.tipFatjar")
      : t("pk.tipClasses"),
);

watch(
  () => ui.pkgOpen,
  async (open) => {
    if (!open) return;
    result.value = null;
    buildError.value = "";
    try {
      [baselines.value, history.value] = await Promise.all([api.pkgBaselines(), api.pkgHistory()]);
    } catch (e) {
      buildError.value = String(e);
    }
  },
);

async function build() {
  building.value = true;
  buildError.value = "";
  result.value = null;
  try {
    result.value = await api.pkgBuild(baseline.value, template.value);
    history.value = await api.pkgHistory();
  } catch (e) {
    buildError.value = String(e);
  } finally {
    building.value = false;
  }
}

async function copyPath() {
  if (result.value) await navigator.clipboard.writeText(result.value.zip_path);
}

function fileName(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
}

</script>

<style scoped>
.pk-modal {
  width: 560px;
  max-height: 78vh;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.pk-title {
  font-weight: 700;
  font-size: var(--font-ui-medium);
  color: var(--text-normal);
}
.pk-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.pk-row > label {
  width: 52px;
  flex-shrink: 0;
  color: var(--text-muted);
}
.pk-select {
  flex: 1;
  min-width: 0;
  height: 30px;
}
.pk-seg {
  display: flex;
  gap: 2px;
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  padding: 2px;
}
.pk-seg-btn {
  height: 24px;
  padding: 0 12px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  font-size: var(--font-ui-size);
}
.pk-seg-btn.is-active {
  background: var(--background-modifier-active-hover);
  color: var(--text-normal);
}
.pk-tip {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  padding-left: 62px;
}
.pk-actions {
  display: flex;
  justify-content: flex-end;
}
.pk-result {
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.pk-result-head {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #4ec970;
}
.pk-zip {
  flex: 1;
  min-width: 0;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}
.pk-copy {
  flex-shrink: 0;
}
.pk-entries {
  max-height: 140px;
  overflow-y: auto;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
}
.pk-entry {
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pk-entry-more {
  color: var(--text-faint);
}
.pk-skipped-title {
  color: #e5a03c;
  font-size: var(--font-ui-smaller);
}
.pk-skipped .pk-entry {
  color: #e5a03c;
}
.pk-error {
  color: var(--text-error, #e93147);
  font-size: var(--font-ui-smaller);
  white-space: pre-wrap;
}
.pk-history-title {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
}
.pk-history-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--font-ui-smaller);
  padding: 2px 0;
  color: var(--text-muted);
}
.pk-h-time {
  flex-shrink: 0;
}
.pk-h-base,
.pk-h-tpl {
  flex-shrink: 0;
  font-family: var(--font-mono);
}
.pk-h-count {
  flex-shrink: 0;
}
.pk-h-zip {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
