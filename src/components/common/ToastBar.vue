<template>
  <!-- 全局瞬时提示（B2：按钮执行失败等需要明确反馈的场景）。
       离场用 emd-fade（anim.css 已有定义，离场时长 ≤ 入场，守动效红线3）。 -->
  <Transition name="emd-fade">
    <!-- aria-live：失败信息对读屏用户必须可闻，否则「静默失败」等于无反馈 -->
    <div
      v-if="ui.toast.msg"
      :key="ui.toast.seq"
      class="ui-toast"
      :class="'is-' + ui.toast.kind"
      role="status"
      aria-live="polite"
    >
      <Icon :name="ui.toast.kind === 'error' ? 'x' : 'check'" :size="13" />
      <span>{{ ui.toast.msg }}</span>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import Icon from "./Icon.vue";
import { useUiStore } from "../../stores/ui";

const ui = useUiStore();
</script>

<style scoped>
.ui-toast {
  position: fixed;
  bottom: 34px; /* 让开底部状态栏 */
  left: 50%;
  transform: translateX(-50%);
  z-index: var(--z-toast);
  display: flex;
  align-items: center;
  gap: 7px;
  max-width: 70vw;
  padding: 7px 14px;
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  background: var(--background-secondary);
  color: var(--text-normal);
  font-size: var(--font-ui-size);
  box-shadow: var(--shadow-l2);
  pointer-events: none;
}
.ui-toast.is-error {
  border-color: var(--text-error);
}
.ui-toast.is-error svg {
  color: var(--text-error);
}
.ui-toast.is-info svg {
  color: var(--text-success);
}
.ui-toast.is-success {
  border-color: var(--text-success);
}
.ui-toast.is-success svg {
  color: var(--text-success);
}
</style>