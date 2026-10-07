<template>
  <div class="dv-body">
    <div v-for="(l, i) in lines" :key="i" class="dv-line" :class="cls(l)">
      <span class="dv-mark">{{ mark(l) }}</span>
      <span class="dv-text">{{ l }}</span>
    </div>
    <div v-if="lines.length === 0" class="dv-empty">{{ t("gt.noDiff") }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { t } from "../../i18n";

const props = defineProps<{ diff: string }>();

const lines = computed(() => props.diff.split("\n").filter((l) => l.length > 0));

function mark(l: string): string {
  if (l.startsWith("+")) return "+";
  if (l.startsWith("-")) return "-";
  if (l.startsWith("@@")) return "@";
  return " ";
}

function cls(l: string): string {
  if (l.startsWith("+++") || l.startsWith("---")) return "is-meta";
  if (l.startsWith("+")) return "is-add";
  if (l.startsWith("-")) return "is-del";
  if (l.startsWith("@@")) return "is-hunk";
  return "";
}
</script>

<style scoped>
.dv-body {
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  line-height: 1.5;
  overflow: auto;
  height: 100%;
  padding: 4px 0;
}
.dv-line {
  display: flex;
  gap: 8px;
  padding: 0 10px;
}
.dv-mark {
  flex-shrink: 0;
  width: 10px;
  color: var(--text-faint);
}
.dv-text {
  white-space: pre-wrap;
  overflow-wrap: break-word;
  color: var(--text-muted);
}
.dv-line.is-add .dv-text {
  color: #4ec970;
  background: rgba(78, 201, 112, 0.08);
}
.dv-line.is-del .dv-text {
  color: var(--text-error, #e93147);
  background: rgba(233, 49, 71, 0.08);
}
.dv-line.is-hunk .dv-text {
  color: var(--interactive-accent);
}
.dv-line.is-meta .dv-text {
  color: var(--text-faint);
}
.dv-empty {
  color: var(--text-faint);
  text-align: center;
  padding: 16px;
}
</style>
