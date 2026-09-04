<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * TUN 虚拟网卡名称配置
 * 作者: TanXiang
 */
import { ref, watch, onUnmounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";

const props = defineProps<{
  initialName: string;
}>();

const settingsStore = useSettingsStore();
const toast = useToast();

const localName = ref(props.initialName);
const saving = ref(false);
const saved = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let savedResetTimer: ReturnType<typeof setTimeout> | null = null;

/** TUN DNS 劫持模式（sing-box 1.14.0 dns_mode：hijack 默认 / disabled 不动系统 DNS） */
const localDnsMode = ref(settingsStore.settings.tun_dns_mode || "hijack");
/** UDP NAT 会话上限（0 = 内核按内存自适应） */
const localUdpNatMax = ref(settingsStore.settings.udp_nat_max || 0);

/** 同步外部 initialName 变化 */
watch(() => props.initialName, (val) => {
  localName.value = val;
});

/** 清理挂起的防抖保存与"已保存"指示器定时器，避免卸载后仍触发保存 */
onUnmounted(() => {
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  if (savedResetTimer) {
    clearTimeout(savedResetTimer);
    savedResetTimer = null;
  }
});

/** 防抖保存 */
function onNameInput() {
  saved.value = false;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(saveName, 800);
}

async function saveName() {
  // blur 与 800ms 防抖都可能触发保存：先取消挂起的定时器，避免一次输入保存两次
  if (saveTimer) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  const trimmed = localName.value.trim();
  if (!trimmed) {
    localName.value = "Auroweave";
  }
  saving.value = true;
  await settingsStore.updateSettings({ tun_interface_name: localName.value });
  saving.value = false;
  saved.value = true;
  if (savedResetTimer) clearTimeout(savedResetTimer);
  savedResetTimer = setTimeout(() => { saved.value = false; }, 2000);
}

async function saveDnsMode() {
  const res = await settingsStore.updateSettings({ tun_dns_mode: localDnsMode.value });
  if (res.success) {
    toast.success("TUN DNS 模式已更新", "重启内核后生效");
  }
}

async function saveUdpNatMax() {
  const val = Math.max(0, Math.min(65535, Math.floor(localUdpNatMax.value || 0)));
  localUdpNatMax.value = val;
  const res = await settingsStore.updateSettings({ udp_nat_max: val });
  if (res.success) {
    toast.success("UDP NAT 上限已更新", val === 0 ? "0 = 内核按内存自适应" : `当前上限: ${val}`);
  }
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
        <span v-else-if="saved" class="status-text saved"><BaseIcon name="Check" :size="13" /> 已保存</span>
      </div>
    </div>

    <div class="setting-item">
      <div class="item-label">
        <span>TUN DNS 模式 (dns_mode)</span>
        <span class="sub-label">sing-box 1.14 起默认劫持系统每接口 DNS；disabled 适合只分流不改 DNS 的场景</span>
      </div>
      <div class="input-wrap">
        <select v-model="localDnsMode" class="text-input select-input" @change="saveDnsMode">
          <option value="hijack">hijack（默认，劫持接口 DNS）</option>
          <option value="disabled">disabled（不接管系统 DNS）</option>
        </select>
      </div>
    </div>

    <div class="setting-item">
      <div class="item-label">
        <span>UDP NAT 会话上限 (udp_nat_max)</span>
        <span class="sub-label">0 = 内核按内存自适应（4096~16384）；游戏/NAT 场景可按需收紧</span>
      </div>
      <div class="input-wrap">
        <input
          v-model.number="localUdpNatMax"
          type="number"
          min="0"
          max="65535"
          class="text-input number-input"
          @blur="saveUdpNatMax"
          @keyup.enter="saveUdpNatMax"
        />
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

.select-input {
  width: 230px;
  cursor: pointer;
}

.number-input {
  width: 100px;
  text-align: right;
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
