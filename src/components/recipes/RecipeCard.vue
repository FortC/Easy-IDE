<template>
  <!-- 配方卡：AI 命令捕获 → 用户确认沉淀 -->
  <div v-if="recipe.proposal" class="emd-modal-bg" @click.self="recipe.rejectProposal()">
    <div class="emd-modal rec-card">
      <div class="rec-title">{{ t("rec.title") }}</div>
      <div class="rec-sub">{{ t("rec.sub") }}</div>

      <div class="rec-field">
        <label>{{ t("rec.action") }}</label>
        <div class="rec-action-chip">{{ actionLabel(recipe.proposal.action) }}</div>
      </div>

      <div v-if="recipe.proposal.candidates.length > 1" class="rec-field">
        <label>{{ t("rec.candidates") }}</label>
        <select
          class="rec-select"
          :value="recipe.proposal.pick"
          @change="onPick($event)"
        >
          <option
            v-for="(c, i) in recipe.proposal.candidates"
            :key="i"
            :value="i"
          >
            [exit {{ c.code ?? "?" }}] {{ c.command }}
          </option>
        </select>
      </div>

      <div class="rec-field">
        <label>{{ t("rec.command") }}</label>
        <textarea v-model="recipe.proposal.command" class="rec-cmd" rows="3" spellcheck="false"></textarea>
      </div>

      <div class="rec-field">
        <label>{{ t("rec.cwd") }}</label>
        <input v-model="recipe.proposal.cwd" class="rec-input" spellcheck="false" />
      </div>

      <div class="rec-field">
        <label>{{ t("rec.ready") }}</label>
        <input
          v-model="recipe.proposal.readyPattern"
          class="rec-input"
          :placeholder="t('rec.readyPh')"
          spellcheck="false"
        />
      </div>

      <div class="rec-field">
        <label>{{ t("rec.files") }}</label>
        <div class="rec-files">
          <span v-for="f in recipe.proposal.fingerprintFiles" :key="f" class="rec-file">{{ f }}</span>
          <span v-if="!recipe.proposal.fingerprintFiles.length" class="rec-no-file">{{ t("rec.noFiles") }}</span>
        </div>
      </div>

      <div class="rec-actions">
        <button class="emd-btn" @click="recipe.rejectProposal()">{{ t("rec.skip") }}</button>
        <button class="emd-btn emd-btn-accent" @click="recipe.confirmProposal()">{{ t("rec.save") }}</button>
      </div>
    </div>
  </div>

  <!-- 轻确认：未信任配方直通前 -->
  <div
    v-else-if="recipe.pendingConfirm"
    class="emd-modal-bg"
    @click.self="recipe.resolveConfirm('cancel')"
  >
    <div class="emd-modal rec-card rec-card-sm">
      <div class="rec-title">{{ t("rec.confirmTitle") }}</div>
      <pre class="rec-cmd-pre">{{ recipe.pendingConfirm.command }}</pre>
      <div class="rec-sub">{{ t("rec.confirmSub") }}</div>
      <div class="rec-actions">
        <button class="emd-btn" @click="recipe.resolveConfirm('cancel')">{{ t("c.cancel") }}</button>
        <button class="emd-btn" @click="recipe.resolveConfirm('once')">{{ t("rec.once") }}</button>
        <button class="emd-btn emd-btn-accent" @click="recipe.resolveConfirm('trust')">
          {{ t("rec.trust") }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRecipeStore } from "../../stores/recipe";
import { t } from "../../i18n";

const recipe = useRecipeStore();

const ACTION_LABELS: Record<string, [string, string]> = {
  run: ["运行", "Run"],
  debug: ["调试", "Debug"],
  build: ["构建", "Build"],
  test: ["测试", "Test"],
  deps: ["依赖", "Deps"],
  clean: ["清理", "Clean"],
};

function actionLabel(action: string): string {
  const pair = ACTION_LABELS[action];
  return pair ? pair[0] : action;
}

function onPick(e: Event) {
  const v = Number((e.target as HTMLSelectElement).value);
  if (!Number.isNaN(v)) recipe.setPick(v);
}
</script>

<style scoped>
.rec-card {
  width: 520px;
  max-width: calc(100vw - 48px);
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.rec-card-sm {
  width: 420px;
}
.rec-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-normal);
}
.rec-sub {
  font-size: var(--font-ui-size);
  color: var(--text-muted);
  line-height: 1.5;
}
.rec-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.rec-field label {
  font-size: var(--font-ui-size);
  color: var(--text-muted);
}
.rec-action-chip {
  display: inline-flex;
  align-self: flex-start;
  padding: 2px 10px;
  border-radius: var(--radius-s);
  background: var(--interactive-accent);
  color: var(--text-on-accent);
  font-size: var(--font-ui-size);
}
.rec-select,
.rec-input,
.rec-cmd {
  width: 100%;
  padding: 5px 8px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-s);
  background: var(--background-primary);
  color: var(--text-normal);
  font-size: var(--font-ui-size);
  font-family: var(--font-monospace);
}
.rec-cmd {
  resize: vertical;
}
.rec-cmd-pre {
  margin: 0;
  padding: 8px 10px;
  border-radius: var(--radius-s);
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  color: var(--text-normal);
  font-family: var(--font-monospace);
  font-size: var(--font-ui-size);
  white-space: pre-wrap;
  word-break: break-all;
}
.rec-files {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.rec-file {
  padding: 1px 6px;
  border-radius: var(--radius-s);
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  color: var(--text-muted);
  font-size: 11px;
  font-family: var(--font-monospace);
}
.rec-no-file {
  color: var(--text-muted);
  font-size: var(--font-ui-size);
}
.rec-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 4px;
}
</style>
