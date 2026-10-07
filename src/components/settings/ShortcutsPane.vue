<template>
  <div class="sc-pane">
    <div class="sc-desc">{{ t("sc.desc") }}</div>
    <div v-for="group in groups" :key="group.title" class="sc-group">
      <div class="sc-group-title">{{ group.title }}</div>
      <div class="sc-row2" v-for="item in group.items" :key="item.key">
        <span class="sc-keys">
          <kbd v-for="(k, i) in item.key.split('+')" :key="i">{{ k }}</kbd>
        </span>
        <span class="sc-what">{{ item.label }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { t } from "../../i18n";

const groups = computed(() => [
  {
    title: t("sc.gFile"),
    items: [
      { key: "Ctrl+P", label: t("sp.file") },
      { key: "Ctrl+N", label: t("sp.class") },
      { key: "Ctrl+Shift+Alt+N", label: t("sp.symbol") },
      { key: "Ctrl+Shift+F", label: t("sp.content") },
      { key: "Ctrl+S", label: t("sc.save") },
    ],
  },
  {
    title: t("sc.gEdit"),
    items: [
      { key: "Ctrl+E", label: t("sc.cycleMode") },
      { key: "Ctrl+G", label: t("sc.graph") },
      { key: "Ctrl+Alt+N", label: t("sc.newNote") },
      { key: "Ctrl+D", label: t("sc.daily") },
    ],
  },
  {
    title: t("sc.gTab"),
    items: [
      { key: "Ctrl+Tab", label: t("sc.nextTab") },
      { key: "Ctrl+Shift+Tab", label: t("sc.prevTab") },
      { key: "Ctrl+W", label: t("tab.close") },
    ],
  },
  {
    title: t("sc.gApp"),
    items: [{ key: "Ctrl+,", label: t("sc.settings") }],
  },
]);
</script>

<style scoped>
.sc-pane {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.sc-desc {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  line-height: 1.6;
}
.sc-group-title {
  color: var(--text-muted);
  font-weight: 600;
  padding: 4px 0;
}
.sc-row2 {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 0;
}
.sc-keys {
  flex-shrink: 0;
  display: flex;
  gap: 4px;
}
kbd {
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-bottom-width: 2px;
  border-radius: var(--radius-s);
  padding: 1px 7px;
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  color: var(--text-normal);
}
.sc-what {
  color: var(--text-muted);
  font-size: var(--font-ui-size);
}
</style>
