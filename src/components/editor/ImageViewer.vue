<template>
  <transition name="emd-fade">
    <div v-if="ui.imagePreview" class="emd-modal-bg" @click.self="close">
      <div class="iv-modal">
        <div class="iv-head">
          <Icon name="image" :size="13" />
          <span class="iv-title">{{ fileName }}</span>
          <button class="iv-tool-btn" :title="t('iv.close')" @click="close">
            <Icon name="x" :size="13" />
          </button>
        </div>
        <div class="iv-body">
          <img :src="src" :alt="fileName" />
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { computed } from "vue";
import Icon from "../common/Icon.vue";
import { useUiStore } from "../../stores/ui";
import { assetUrl } from "../../ipc/tauri";
import { t } from "../../i18n";

const ui = useUiStore();

const src = computed(() => assetUrl(ui.imagePreview));
const fileName = computed(() => ui.imagePreview.split("/").pop() || ui.imagePreview);

function close() {
  ui.imagePreview = "";
}
</script>

<style scoped>
.iv-modal {
  width: min(860px, 86vw);
  height: min(640px, 82vh);
  padding: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.iv-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--background-modifier-border);
  flex-shrink: 0;
  color: var(--text-muted);
}
.iv-title {
  flex: 1;
  font-size: var(--font-ui-smaller);
  color: var(--text-normal);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.iv-tool-btn {
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-s);
  color: var(--text-muted);
}
.iv-tool-btn:hover {
  background: var(--background-modifier-hover);
  color: var(--text-normal);
}
.iv-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 12px;
  overflow: auto;
}
.iv-body img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border-radius: var(--radius-s);
}
</style>
