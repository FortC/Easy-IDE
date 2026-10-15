<template>
  <transition name="emd-fade">
    <BaseModal :title="title" width="sm" :show-close="false" @close="cancel">
      <div class="id-body">
        <input
          ref="inputRef"
          v-model="value"
          type="text"
          :placeholder="placeholder"
          @keydown.enter="confirm"
        />
      </div>
      <template #footer>
        <button class="emd-btn" @click="cancel">{{ t("c.cancel") }}</button>
        <button class="emd-btn emd-btn-accent" :disabled="!value.trim()" @click="confirm">
          {{ t("c.confirm") }}
        </button>
      </template>
    </BaseModal>
  </transition>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";
import BaseModal from "./BaseModal.vue";
import { t } from "../../i18n";

const props = withDefaults(
  defineProps<{ title: string; placeholder?: string; initial?: string }>(),
  { placeholder: "", initial: "" },
);
const emit = defineEmits<{ confirm: [value: string]; cancel: [] }>();

const value = ref(props.initial);
const inputRef = ref<HTMLInputElement>();

onMounted(() => {
  void nextTick(() => {
    inputRef.value?.focus();
    inputRef.value?.select();
  });
});

function confirm() {
  if (value.value.trim()) emit("confirm", value.value.trim());
}
function cancel() {
  emit("cancel");
}
</script>

<style scoped>
.id-body {
  padding: var(--space-5);
}
</style>
