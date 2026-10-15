<script setup lang="ts">
/**
 * 轻量 Tooltip：替代原生 :title。
 * 原生 title 有三个问题 —— 无延迟（鼠标扫过就闪）、不能自定义样式、触发慢。
 * 用法：<BaseTooltip text="项目结构" side="right"><button/></BaseTooltip>
 */
import { computed, onBeforeUnmount, ref } from "vue";

const props = withDefaults(
  defineProps<{
    text: string;
    side?: "top" | "bottom" | "left" | "right";
    /** 出现延迟，0 表示立即 */
    delay?: number;
  }>(),
  { side: "bottom", delay: 400 },
);

const open = ref(false);
let timer: number | undefined;

function show() {
  window.clearTimeout(timer);
  if (props.delay === 0) open.value = true;
  else timer = window.setTimeout(() => (open.value = true), props.delay);
}
function hide() {
  window.clearTimeout(timer);
  open.value = false;
}
onBeforeUnmount(hide);

const style = computed(() => {
  const map = {
    top: "bottom: 100%; left: 50%; transform: translate(-50%, -4px)",
    bottom: "top: 100%; left: 50%; transform: translate(-50%, 4px)",
    left: "right: 100%; top: 50%; transform: translate(-4px, -50%)",
    right: "left: 100%; top: 50%; transform: translate(4px, -50%)",
  };
  return map[props.side];
});
</script>

<template>
  <span class="emd-tip-wrap" @mouseenter="show" @mouseleave="hide" @mousedown="hide">
    <slot />
    <transition name="emd-pop">
      <span v-if="open && text" class="emd-tip" role="tooltip" :style="style">{{ text }}</span>
    </transition>
  </span>
</template>

<style>
.emd-tip-wrap {
  position: relative;
  display: inline-flex;
}
.emd-tip {
  position: absolute;
  z-index: var(--z-dropdown);
  padding: 3px var(--space-2);
  border-radius: var(--radius-s);
  background: var(--background-modifier-hover);
  color: var(--text-normal);
  font-size: var(--font-ui-smaller);
  line-height: 1.4;
  white-space: nowrap;
  pointer-events: none;
  box-shadow: var(--shadow-l1);
}
</style>
