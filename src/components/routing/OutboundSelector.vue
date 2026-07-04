<template>
  <div class="outbound-selector-wrapper">
    <select
      :value="modelValue || 'default'"
      class="outbound-select"
      @change="handleChange"
    >
      <option value="default">⚙️ 跟随默认规则</option>
      <option value="direct">➡️ 直连模式 (Direct)</option>
      <option value="block">🚫 拦截阻止 (Block)</option>
      <optgroup label="── 代理分组 ──">
        <option
          v-for="group in groups"
          :key="group.tag"
          :value="group.tag"
        >
          🌐 {{ group.tag }}
        </option>
      </optgroup>
    </select>
  </div>
</template>

<script setup lang="ts">
import { useProxyStore } from "@/stores/proxy.store";
import { storeToRefs } from "pinia";

defineProps<{
  modelValue?: string;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", val: string): void;
  (e: "change", val: string): void;
}>();

const proxyStore = useProxyStore();
const { groups } = storeToRefs(proxyStore);

function handleChange(e: Event) {
  const target = e.target as HTMLSelectElement;
  const val = target.value;
  emit("update:modelValue", val);
  emit("change", val);
}
</script>

<style scoped>
.outbound-selector-wrapper {
  position: relative;
  display: inline-block;
}

.outbound-select {
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.9);
  font-size: 13px;
  outline: none;
  cursor: pointer;
  transition: all 0.2s ease;
}

.outbound-select:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: #00f2fe;
}

option, optgroup {
  background: #121622;
  color: #fff;
}
</style>
