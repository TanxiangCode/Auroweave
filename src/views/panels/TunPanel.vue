<script setup lang="ts">
/**
 * TUN 网卡配置面板
 * 作者: TanXiang
 */
import { ref, onMounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";

const settingsStore = useSettingsStore();

// 本地编辑值，防止每次按键都写入磁盘
const localName = ref(settingsStore.settings.tun_interface_name || "Auroweave");
const saving = ref(false);
const saved = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

// 同步 store 变化到本地（首次加载设置后）
onMounted(() => {
  localName.value = settingsStore.settings.tun_interface_name || "Auroweave";
});

// 防抖保存：用户停止输入 800ms 后自动保存
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
  <div class="panel-container">
    <h2>🔌 TUN 虚拟网卡</h2>
    <p class="panel-desc">
      TUN 模式通过虚拟网卡在系统内核层接管流量，需要管理员权限。
      首次启用时将弹出一次 UAC 授权窗口，之后无需重复授权。
    </p>

    <div class="setting-group">
      <!-- 网卡接口名 -->
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

      <!-- 提示卡片 -->
      <div class="info-card">
        <div class="info-icon">💡</div>
        <div class="info-body">
          <p><strong>为什么只需授权一次？</strong></p>
          <p>
            首次启用 TUN 时，Auroweave 会将 sing-box 注册为 Windows 系统服务（SYSTEM 权限），
            并自动授予当前用户对该服务的启动/停止权限。之后切换 TUN 模式无需再次 UAC 授权，
            与 Mihomo、Clash Verge 等工具完全相同。
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

h2 {
  font-size: 18px;
  font-weight: 700;
}

.panel-desc {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  line-height: 1.6;
  margin: 0;
}

.setting-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
}

.item-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 14px;
  font-weight: 600;
}

.sub-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  font-weight: normal;
  max-width: 280px;
  line-height: 1.4;
}

.input-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.text-input {
  padding: 7px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.9);
  font-size: 13px;
  width: 150px;
  outline: none;
  transition: border-color 0.15s;
}

.text-input:focus {
  border-color: rgba(0, 242, 254, 0.4);
}

.status-text {
  font-size: 11px;
  white-space: nowrap;
}

.saving {
  color: rgba(255, 255, 255, 0.4);
}

.saved {
  color: #4ade80;
}

/* 提示卡片 */
.info-card {
  display: flex;
  gap: 12px;
  padding: 14px 16px;
  background: rgba(0, 242, 254, 0.05);
  border: 1px solid rgba(0, 242, 254, 0.15);
  border-radius: 12px;
}

.info-icon {
  font-size: 18px;
  flex-shrink: 0;
  padding-top: 1px;
}

.info-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.info-body p {
  margin: 0;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
  line-height: 1.6;
}

.info-body strong {
  color: rgba(0, 242, 254, 0.9);
}
</style>
