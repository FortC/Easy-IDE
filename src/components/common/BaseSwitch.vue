<script setup lang="ts">
/** 统一开关。此前 6 处 checkbox/switch 用浏览器默认外观裸奔。 */
const props = defineProps<{ modelValue: boolean; disabled?: boolean; ariaLabel?: string }>();
const emit = defineEmits<{ "update:modelValue": [boolean] }>();
</script>

<template>
  <button
    class="emd-switch"
    :class="{ 'is-on': modelValue }"
    role="switch"
    :aria-checked="modelValue"
    :aria-label="ariaLabel"
    :disabled="disabled"
    :tabindex="disabled ? -1 : 0"
    @click="emit('update:modelValue', !props.modelValue)"
    @keydown.space.prevent="emit('update:modelValue', !props.modelValue)"
    @keydown.enter.prevent="emit('update:modelValue', !props.modelValue)"
  />
</template>

<style>
.emd-switch:disabled {
  opacity: 0.45;
  pointer-events: none;
}
</style>
