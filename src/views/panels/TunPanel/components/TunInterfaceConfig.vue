<script setup lang="ts">
/**
 * TUN 虚拟网卡名称配置
 * 作者: TanXiang
 */
import { ref, watch } from "vue";
import { useSettingsStore } from "@/stores/settings.store";

const props = defineProps<{
  initialName: string;
}>();

const settingsStore = useSettingsStore();

const localName = ref(props.initialName);
const saving = ref(false);
const saved = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

/** 同步外部 initialName 变化 */
watch(() => props.initialName, (val) => {
  localName.value = val;
});

/** 防抖保存 */
function onNameInput() {
  saved.value = false;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(saveName, 800);
}

async function saveName() {
  const trimmed = localName.value.trim();
  if (!trimmed) {
    localName.value = "Auroweave";
  }
  saving.value = true;
  await settingsStore.updateSettings({ tun_interface_name: localName.value });
  saving.value = false;
  saved.value = true;
  setTimeout(() => { saved.value = false; }, 2000);
}
</script>

<template>
  <div class="setting-group">
    <div class="setting-item">
      <div class="item-label">
        <span>虚拟网卡名称</span>
        <span class="sub-label">显示在 Windows 网络适配器列表中，修改后重启 TUN 生效</span>
      </div>
      <div class="input-wrap">
        <input
          id="tun-interface-name"
          v-model="localName"
          type="text"
          class="text-input"
          placeholder="Auroweave"
          maxlength="32"
          @input="onNameInput"
          @blur="saveName"
        />
        <span v-if="saving" class="status-text saving">保存中…</span>
        <span v-else-if="saved" class="status-text saved">✓ 已保存</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.input-wrap {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.text-input {
  padding: 7px var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  width: 150px;
  outline: none;
  transition: border-color var(--duration-fast);
}

.text-input:focus {
  border-color: var(--accent-cyan-vivid);
}

.status-text {
  font-size: var(--text-xs);
  white-space: nowrap;
}

.saving {
  color: var(--text-tertiary);
}

.saved {
  color: var(--status-success);
}
</style>
