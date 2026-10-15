<template>
  <teleport to="body">
    <transition name="emd-fade">
      <div
        v-if="open"
        class="emd-dd-backdrop"
        @pointerdown="emit('close')"
        @contextmenu.prevent="emit('close')"
      />
    </transition>
    <transition name="emd-pop">
      <div v-if="open" ref="menuEl" class="emd-dropdown" :style="pos" role="menu" @pointerdown.stop>
        <div v-if="title" class="emd-dd-title">{{ title }}</div>
        <template v-for="item in items" :key="item.key">
          <div v-if="item.separator" class="emd-dd-sep" />
          <button
            v-else
            class="emd-dd-item"
            :class="{ 'is-danger': item.danger, 'is-cursor': cursorKey === item.key }"
            role="menuitem"
            @mouseenter="cursorKey = item.key"
            @click="emit('select', item.key)"
          >
            <Icon v-if="item.icon" :name="item.icon" :size="14" />
            <span class="emd-dd-label">{{ item.label }}</span>
            <span v-if="item.hint" class="emd-dd-hint">{{ item.hint }}</span>
          </button>
        </template>
        <slot />
      </div>
    </transition>
  </teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import Icon from "./Icon.vue";

export interface DropItem {
  key: string;
  label: string;
  icon?: string;
  hint?: string;
  danger?: boolean;
  /** 渲染为分隔线 */
  separator?: boolean;
}

const props = withDefaults(
  defineProps<{
    open: boolean;
    items?: DropItem[];
    /** 锚定元素（菜单出现在其下方） */
    anchor?: HTMLElement | null;
    /** 或直接指定坐标（右键菜单） */
    x?: number;
    y?: number;
    align?: "left" | "right";
    title?: string;
  }>(),
  { items: () => [], anchor: null, x: undefined, y: undefined, align: "left", title: "" },
);
const emit = defineEmits<{ select: [key: string]; close: [] }>();

const menuEl = ref<HTMLElement>();
const pos = ref({ left: "-9999px", top: "-9999px" });
/** 键盘高亮项（null = 未用键盘，仍按 hover 走） */
const cursorKey = ref<string | null>(null);
let opener: HTMLElement | null = null;

/** 可用菜单项（跳过分隔线） */
function usableKeys(): string[] {
  return props.items.filter((i) => !i.separator).map((i) => i.key);
}

function moveCursor(dir: 1 | -1) {
  const keys = usableKeys();
  if (!keys.length) return;
  const i = cursorKey.value ? keys.indexOf(cursorKey.value) : -1;
  const next = keys[(i + dir + keys.length) % keys.length];
  cursorKey.value = next;
  requestAnimationFrame(() => {
    menuEl.value?.querySelector<HTMLElement>(`.emd-dd-item.is-cursor`)?.scrollIntoView({ block: "nearest" });
  });
}

/** 视口内自动翻转定位 */
function place() {
  const el = menuEl.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  let left: number;
  let top: number;
  if (props.anchor) {
    const r = props.anchor.getBoundingClientRect();
    top = r.bottom + 6;
    left = props.align === "right" ? r.right - rect.width : r.left;
    // 底部放不下 → 翻到锚点上方
    if (top + rect.height > vh - 8) top = r.top - rect.height - 6;
  } else {
    top = props.y ?? 0;
    left = props.x ?? 0;
    if (top + rect.height > vh - 8) top = Math.max(8, vh - rect.height - 8);
  }
  if (left + rect.width > vw - 8) left = vw - rect.width - 8;
  if (left < 8) left = 8;
  if (top < 8) top = 8;
  pos.value = { left: `${left}px`, top: `${top}px` };
}

watch(
  () => props.open,
  async (v) => {
    if (v) {
      opener = document.activeElement as HTMLElement | null;
      await nextTick();
      place();
      window.addEventListener("keydown", onKey);
    } else {
      cursorKey.value = null;
      window.removeEventListener("keydown", onKey);
      // 关闭后把焦点还给触发元素
      opener?.focus?.();
      opener = null;
    }
  },
);
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  opener?.focus?.();
});

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") {
    emit("close");
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveCursor(1);
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    moveCursor(-1);
    return;
  }
  if (e.key === "Enter" && cursorKey.value) {
    e.preventDefault();
    emit("select", cursorKey.value);
  }
}
</script>

<style scoped>
.emd-dd-backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--z-dropdown);
}
.emd-dropdown {
  position: fixed;
  z-index: var(--z-dropdown);
  background: var(--background-secondary);
  border: 1px solid var(--background-modifier-border);
  border-radius: var(--radius-m);
  box-shadow: var(--shadow-l2);
  padding: 4px;
  min-width: 210px;
  max-width: 320px;
}
.emd-dd-title {
  font-size: var(--font-ui-smaller);
  color: var(--text-faint);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 6px 10px 4px;
}
.emd-dd-sep {
  height: 1px;
  background: var(--background-modifier-border);
  margin: 4px 6px;
}
.emd-dd-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: 6px 10px;
  border-radius: var(--radius-s);
  color: var(--text-normal);
  font-size: var(--font-ui-size);
  text-align: left;
  white-space: nowrap;
  transition: background var(--anim-hover), transform var(--anim-press);
}
.emd-dd-item:hover,
.emd-dd-item.is-cursor {
  background: var(--background-modifier-hover);
}
.emd-dd-item:active {
  transform: scale(0.98);
}
@media (hover: none) {
  .emd-dd-item:active {
    transform: none;
  }
}
.emd-dd-item.is-danger {
  color: var(--text-error);
}
.emd-dd-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
}
.emd-dd-hint {
  color: var(--text-faint);
  font-size: var(--font-ui-smaller);
  margin-left: auto;
  padding-left: 16px;
}
</style>
