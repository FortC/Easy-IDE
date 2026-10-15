<script setup lang="ts">
/**
 * 统一模态基座（Obsidian 风格：居中、圆角 12px、--background-secondary）。
 * 解决此前 7 处手写重复 DOM 且无障碍缺失的问题：
 *  - 焦点陷阱：Tab 不会跑出弹窗
 *  - Esc 全局监听：不再依赖「恰好焦点在某个 input 上」
 *  - 关闭后焦点归还触发元素
 *  - role="dialog" aria-modal
 */
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

const props = withDefaults(
  defineProps<{
    /** 标题；为空则渲染无标题栏（命令面板等形态） */
    title?: string;
    /** 宽度：sm 420 / md 560 / lg 760 */
    width?: "sm" | "md" | "lg" | "full";
    /** 点击遮罩是否关闭 */
    closeOnBackdrop?: boolean;
    /** 标题栏是否显示右上角关闭按钮 */
    showClose?: boolean;
    ariaLabel?: string;
  }>(),
  { width: "md", closeOnBackdrop: true, showClose: true },
);

const emit = defineEmits<{ close: [] }>();

const panel = ref<HTMLElement | null>(null);
let opener: HTMLElement | null = null;

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.stopPropagation();
    e.preventDefault();
    emit("close");
    return;
  }
  if (e.key !== "Tab" || !panel.value) return;
  const items = [...panel.value.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => el.offsetParent !== null,
  );
  if (!items.length) return;
  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement as HTMLElement | null;
  // 焦点不在弹窗内（刚打开或用户点了遮罩）→ 拉回首项
  if (!active || !panel.value.contains(active)) {
    e.preventDefault();
    first.focus();
    return;
  }
  if (e.shiftKey && active === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && active === last) {
    e.preventDefault();
    first.focus();
  }
}

// 打开时记住触发元素并把焦点送进弹窗；关闭时归还
function focusFirst() {
  if (!panel.value) return;
  const items = [...panel.value.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => el.offsetParent !== null,
  );
  (items[0] ?? panel.value).focus();
}

onMounted(() => {
  opener = document.activeElement as HTMLElement | null;
  document.addEventListener("keydown", onKeydown, true);
  void nextTick(focusFirst);
});

onBeforeUnmount(() => {
  document.removeEventListener("keydown", onKeydown, true);
  opener?.focus?.();
});

function onBackdrop() {
  if (props.closeOnBackdrop) emit("close");
}
</script>

<template>
  <div class="emd-modal-bg" :class="`is-${width}`" @mousedown.self="onBackdrop">
    <div
      ref="panel"
      class="emd-modal"
      role="dialog"
      aria-modal="true"
      :aria-label="ariaLabel ?? title"
      tabindex="-1"
    >
      <div v-if="title" class="emd-modal-title">
        <span>{{ title }}</span>
        <button v-if="showClose" class="emd-modal-x" :aria-label="'关闭'" @click="emit('close')">
          <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M18 6 6 18M6 6l12 12"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            />
          </svg>
        </button>
      </div>
      <div class="emd-modal-body">
        <slot />
      </div>
      <div v-if="$slots.footer" class="emd-modal-foot">
        <slot name="footer" />
      </div>
    </div>
  </div>
</template>

<style>
.emd-modal-bg {
  z-index: var(--z-modal);
}
.emd-modal-bg.is-sm .emd-modal {
  width: 420px;
}
.emd-modal-bg.is-lg .emd-modal {
  width: 760px;
}
.emd-modal-bg.is-full .emd-modal {
  width: min(1080px, 94vw);
  max-height: 88vh;
}
.emd-modal-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--background-modifier-border);
  font-size: var(--font-ui-medium);
}
.emd-modal-x {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-s);
  color: var(--text-muted);
  transition: background var(--anim-hover), color var(--anim-hover), transform var(--anim-press);
}
.emd-modal-x:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.emd-modal-x:active {
  transform: scale(0.92);
}
@media (hover: none) {
  .emd-modal-x:active {
    transform: none;
  }
}
.emd-modal-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
}
.emd-modal-foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-5);
  border-top: 1px solid var(--background-modifier-border);
}
</style>
