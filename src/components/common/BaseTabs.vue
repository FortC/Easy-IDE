<script setup lang="ts">
/**
 * 统一页签基座。此前右栏 / 设置 / 控制台 / 搜索面板各有 4 套手写实现，
 * 全部缺 role="tablist" 与键盘方向键导航。
 */
import { ref } from "vue";

export interface TabItem {
  key: string;
  label: string;
  /** 右侧徽标数字（如问题数量） */
  badge?: number | string;
  icon?: string;
  disabled?: boolean;
}

const props = defineProps<{
  modelValue: string;
  items: TabItem[];
  /** small 用于控制台窄页签，default 用于右栏/设置 */
  size?: "sm" | "default";
  ariaLabel?: string;
}>();

const emit = defineEmits<{ "update:modelValue": [string] }>();

const root = ref<HTMLElement | null>(null);

function onKeydown(e: KeyboardEvent) {
  const dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
  if (!dir) return;
  e.preventDefault();
  const usable = props.items.filter((i) => !i.disabled);
  if (!usable.length) return;
  const i = usable.findIndex((it) => it.key === props.modelValue);
  const next = usable[(i + dir + usable.length) % usable.length];
  emit("update:modelValue", next.key);
  // 焦点跟随：Roving tabindex
  void nextTickFocus(next.key);
}

function nextTickFocus(key: string) {
  requestAnimationFrame(() => {
    root.value?.querySelector<HTMLElement>(`[data-key="${key}"]`)?.focus();
  });
}

function pick(key: string, disabled?: boolean) {
  if (disabled) return;
  emit("update:modelValue", key);
}
</script>

<template>
  <div
    ref="root"
    class="emd-tabs"
    :class="`is-${size}`"
    role="tablist"
    :aria-label="ariaLabel"
    @keydown="onKeydown"
  >
    <button
      v-for="it in items"
      :key="it.key"
      class="emd-tab"
      :class="{ 'is-active': it.key === modelValue }"
      :data-key="it.key"
      role="tab"
      :aria-selected="it.key === modelValue"
      :disabled="it.disabled"
      :tabindex="it.key === modelValue ? 0 : -1"
      @click="pick(it.key, it.disabled)"
    >
      <span class="emd-tab-label">{{ it.label }}</span>
      <span v-if="it.badge !== undefined && it.badge !== 0" class="emd-tab-badge">{{ it.badge }}</span>
    </button>
  </div>
</template>

<style>
.emd-tabs {
  display: flex;
  align-items: stretch;
  gap: var(--space-1);
}
.emd-tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: 26px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-s);
  font-size: var(--font-ui-smaller);
  color: var(--text-muted);
  white-space: nowrap;
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.emd-tab:hover {
  color: var(--text-normal);
  background: var(--background-modifier-hover);
}
.emd-tab.is-active {
  color: var(--text-normal);
  background: var(--background-modifier-active-hover);
}
.emd-tab:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .emd-tab:active {
    transform: none;
  }
}
.emd-tabs.is-sm .emd-tab {
  height: 24px;
  padding: 0 var(--space-2);
}
.emd-tab-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 15px;
  height: 15px;
  padding: 0 4px;
  border-radius: 8px;
  background: var(--background-modifier-border);
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1;
}
.emd-tab.is-active .emd-tab-badge {
  background: var(--interactive-accent);
  color: var(--text-on-accent);
}
</style>
