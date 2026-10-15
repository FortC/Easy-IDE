<template>
  <transition name="emd-fade">
    <div v-if="visible" class="emd-modal-bg" @click.self="emit('select', 'cancel')">
      <div class="emd-modal oc-modal">
        <div class="oc-header">
          <svg viewBox="0 0 48 48" width="40" height="40" class="oc-logo">
            <rect x="4" y="4" width="40" height="40" rx="10" fill="var(--interactive-accent)" />
            <path d="M17 14h4v20h-4zM17 14h14v4H17zM17 23h11v4H17zM17 30h14v4H17z" fill="#fff" />
          </svg>
          <div class="oc-header-text">
            <div class="oc-title">EasyIDE</div>
            <div class="oc-subtitle">{{ t("oc.title") }}</div>
          </div>
        </div>

        <div class="oc-file-box">
          <svg viewBox="0 0 24 24" width="18" height="18" class="oc-file-icon">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" fill="var(--text-accent)" opacity="0.3"/>
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" fill="none" stroke="var(--text-accent)" stroke-width="1.5"/>
            <polyline points="14 2 14 8 20 8" fill="none" stroke="var(--text-accent)" stroke-width="1.5"/>
          </svg>
          <span class="oc-file-name">{{ fileName }}</span>
        </div>

        <div class="oc-cards">
          <button class="oc-card" @click="emit('select', 'vault')">
            <div class="oc-card-icon">
              <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20"/>
              </svg>
            </div>
            <div class="oc-card-body">
              <div class="oc-card-title">{{ t("oc.vault") }}</div>
              <div class="oc-card-desc">{{ t("oc.vaultDesc") }}</div>
            </div>
          </button>

          <button class="oc-card oc-card-temp" @click="emit('select', 'temp')">
            <div class="oc-card-icon">
              <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14 2 14 8 20 8"/>
                <line x1="9" y1="15" x2="15" y2="15"/>
              </svg>
            </div>
            <div class="oc-card-body">
              <div class="oc-card-title">{{ t("oc.temp") }}</div>
              <div class="oc-card-desc">{{ t("oc.tempDesc") }}</div>
            </div>
          </button>
        </div>

        <button class="oc-cancel" @click="emit('select', 'cancel')">{{ t("c.cancel") }}</button>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { t } from "../../i18n";

defineProps<{ visible: boolean; fileName: string }>();
const emit = defineEmits<{ select: [mode: "vault" | "temp" | "cancel"] }>();
</script>

<style scoped>
/* ---------- 打开文件弹窗（懒加载：仅系统打开文件场景，样式不进首屏包） ---------- */
.oc-modal {
  width: 460px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.oc-header {
  display: flex;
  align-items: center;
  gap: 14px;
}
.oc-logo {
  flex-shrink: 0;
  border-radius: 10px;
}
.oc-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-normal);
  letter-spacing: 0.02em;
}
.oc-subtitle {
  font-size: 13px;
  color: var(--text-muted);
  margin-top: 2px;
}
.oc-file-box {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: var(--radius-m);
  background: var(--background-primary);
  border: 1px solid var(--background-modifier-border);
}
.oc-file-icon {
  flex-shrink: 0;
}
.oc-file-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-normal);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.oc-cards {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.oc-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 16px;
  border-radius: var(--radius-l);
  border: 2px solid var(--background-modifier-border);
  background: var(--background-primary);
  text-align: left;
  cursor: pointer;
  transition: border-color var(--anim-fast), background var(--anim-fast), transform var(--anim-fast);
}
.oc-card:hover {
  border-color: var(--interactive-accent);
  background: var(--interactive-accent-hover-alt);
  transform: translateY(-1px);
}
.oc-card:active {
  transform: scale(0.96);
  transition: transform var(--anim-press);
}
.oc-card-icon {
  width: 44px;
  height: 44px;
  border-radius: var(--radius-m);
  background: var(--interactive-accent-hover-alt);
  color: var(--interactive-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.oc-card:hover .oc-card-icon {
  background: var(--interactive-accent);
  color: var(--text-on-accent);
}
.oc-card-temp .oc-card-icon {
  background: var(--code-background);
  color: var(--text-muted);
}
.oc-card-temp:hover .oc-card-icon {
  background: var(--text-muted);
  color: var(--text-on-accent);
}
.oc-card-body {
  flex: 1;
  min-width: 0;
}
.oc-card-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-normal);
  margin-bottom: 3px;
}
.oc-card-desc {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.4;
}
.oc-cancel {
  align-self: center;
  padding: 6px 20px;
  border-radius: var(--radius-m);
  color: var(--text-faint);
  font-size: 13px;
  transition: color var(--anim-fast);
}
.oc-cancel:hover {
  color: var(--text-normal);
}
</style>
