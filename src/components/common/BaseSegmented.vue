<script setup lang="ts">
/**
 * 统一分段控件。此前有 3 份手写实现（st-seg / ep-seg / pk-seg），
 * 样式与键盘行为各不相同。方向键在选项间移动。
 */
import { ref } from "vue";

export interface SegOption {
  value: string;
  label: string;
  title?: string;
}

const props = defineProps<{ modelValue: string; options: SegOption[]; ariaLabel?: string }>();
const emit = defineEmits<{ "update:modelValue": [string] }>();

const root = ref<HTMLElement | null>(null);

function onKeydown(e: KeyboardEvent) {
  const dir = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
  if (!dir) return;
  e.preventDefault();
  const i = props.options.findIndex((o) => o.value === props.modelValue);
  const next = props.options[(i + dir + props.options.length) % props.options.length];
  emit("update:modelValue", next.value);
  requestAnimationFrame(() => {
    root.value?.querySelector<HTMLElement>(`[data-value="${next.value}"]`)?.focus();
  });
}
</script>

<template>
  <div ref="root" class="emd-seg" role="radiogroup" :aria-label="ariaLabel" @keydown="onKeydown">
    <button
      v-for="o in options"
      :key="o.value"
      class="emd-seg-btn"
      :class="{ 'is-active': o.value === modelValue }"
      :data-value="o.value"
      role="radio"
      :aria-checked="o.value === modelValue"
      :title="o.title"
      :tabindex="o.value === modelValue ? 0 : -1"
      @click="emit('update:modelValue', o.value)"
    >
      {{ o.label }}
    </button>
  </div>
</template>

<style>
.emd-seg .emd-seg-btn:active {
  transform: scale(0.96);
}
@media (hover: none) {
  .emd-seg .emd-seg-btn:active {
    transform: none;
  }
}
</style>
