<script setup lang="ts">
/**
 * 统一按钮基座。所有按钮都必须有按压反馈（AGENTS.md 高频交互红线）。
 * 组件样式不带 scoped —— :active / :focus-visible 需要跨组件稳定命中，
 * 按压变换写在组件内的全局选择器里（见 <style>，非 scoped）。
 */
withDefaults(
  defineProps<{
    variant?: "default" | "accent" | "ghost" | "danger";
    size?: "md" | "sm";
    disabled?: boolean;
    /** 无边框幽灵按钮（工具条图标按钮） */
    iconOnly?: boolean;
    type?: "button" | "submit";
    title?: string;
    ariaLabel?: string;
    pressed?: boolean;
  }>(),
  { variant: "default", size: "md", type: "button" },
);
</script>

<template>
  <button
    class="emd-btn"
    :class="[`is-${variant}`, size === 'sm' && 'emd-btn-sm', iconOnly && 'is-icon']"
    :type="type"
    :disabled="disabled"
    :title="title"
    :aria-label="ariaLabel"
    :aria-pressed="pressed"
  >
    <slot />
  </button>
</template>

<style>
/* 变体（覆盖 theme.css 基座里已有的 accent，其余新增） */
.emd-btn.is-ghost {
  background: transparent;
  color: var(--text-muted);
}
.emd-btn.is-ghost:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.emd-btn.is-danger {
  background: transparent;
  color: var(--text-error);
  border: 1px solid var(--background-modifier-border);
}
.emd-btn.is-danger:hover {
  background: var(--background-modifier-hover);
  border-color: var(--text-error);
}
.emd-btn.is-icon {
  padding: 0;
  width: var(--input-height);
}
.emd-btn.is-icon.emd-btn-sm {
  width: 24px;
}

/* 按压反馈：transform 走合成器、立即生效（红线1/高频按压红线） */
.emd-btn:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .emd-btn:active {
    transform: none;
  }
}
</style>
