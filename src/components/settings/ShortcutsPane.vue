<template>
  <div class="sc-pane">
    <div class="sc-desc">{{ t("sc.desc") }}</div>
    <div class="sc-toolbar">
      <button class="emd-btn" @click="kb.resetAll()">
        <Icon name="refresh-cw" :size="12" /> {{ t("sc.resetAll") }}
      </button>
    </div>
    <div class="sc-group">
      <div class="sc-row2" v-for="a in actions" :key="a.id">
        <span class="sc-what" :class="{ 'is-conflict': conflicts[a.id] }">{{ a.label }}</span>
        <button
          class="sc-key-btn"
          :class="{ 'is-listening': listening === a.id, 'is-conflict': conflicts[a.id] }"
          @click="startListen(a.id)"
          @keydown="onCapture($event, a.id)"
        >
          <template v-if="listening === a.id">{{ t("sc.listening") }}</template>
          <template v-else>{{ kb.bindings[a.id] || a.def }}</template>
        </button>
        <button
          v-if="kb.bindings[a.id] !== normalize(a.def)"
          class="sc-reset"
          :title="t('sc.resetOne')"
          @click="kb.resetBinding(a.id)"
        >
          <Icon name="eraser" :size="12" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import Icon from "../common/Icon.vue";
import { KB_ACTIONS, normalizeSeq, seqFromEvent, useKeybindingsStore } from "../../stores/keybindings";
import { t } from "../../i18n";

const kb = useKeybindingsStore();
const listening = ref("");

const actions = computed(() =>
  KB_ACTIONS.map((a) => ({ id: a.id, label: a.label(), def: a.def })),
);
const conflicts = computed(() => kb.conflicts());

const normalize = (seq: string) => normalizeSeq(seq);

function startListen(id: string) {
  listening.value = id;
}

async function onCapture(e: KeyboardEvent, id: string) {
  if (listening.value !== id) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    listening.value = "";
    return;
  }
  const seq = seqFromEvent(e);
  if (!seq) return; // 纯修饰键：继续等待
  if (!seq.includes("+") || seq.startsWith("shift+") || seq.startsWith("alt+")) {
    // 至少要有一个主修饰（Ctrl），避免吃掉普通按键
    return;
  }
  listening.value = "";
  await kb.setBinding(id, seq);
}

// 失焦/点击别处取消监听
function onBlur() {
  listening.value = "";
}
onMounted(() => window.addEventListener("pointerdown", onBlur));
onUnmounted(() => window.removeEventListener("pointerdown", onBlur));
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
.sc-toolbar {
  display: flex;
  justify-content: flex-end;
}
.sc-row2 {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 0;
}
.sc-what {
  flex: 1;
  color: var(--text-muted);
  font-size: var(--font-ui-size);
}
.sc-what.is-conflict,
.sc-key-btn.is-conflict {
  color: var(--text-error, #e93147);
}
.sc-key-btn {
  min-width: 150px;
  padding: 3px 10px;
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
  border-bottom-width: 2px;
  border-radius: var(--radius-s);
  font-family: var(--font-mono);
  font-size: var(--font-ui-smaller);
  color: var(--text-normal);
  text-align: center;
}
.sc-key-btn:hover {
  border-color: var(--interactive-accent);
}
.sc-key-btn.is-listening {
  border-color: var(--interactive-accent);
  color: var(--interactive-accent);
  animation: sc-pulse 1s ease-in-out infinite;
}
@keyframes sc-pulse {
  50% {
    opacity: 0.55;
  }
}
.sc-reset {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-faint);
}
.sc-reset:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
</style>
