<template>
  <div class="panel-container">
    <h2>🔀 代理模式与端口设置</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>代理分流模式</span>
          <span class="sub-label">切换当前全局路由分流规则</span>
        </div>
        <select v-model="settingsStore.settings.proxy_mode" class="select-input" @change="saveMode">
          <option value="rule">🔀 规则模式 (Rule)</option>
          <option value="global">🌐 全局代理 (Global)</option>
          <option value="direct">⚡ 直连模式 (Direct)</option>
        </select>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>本地混合代理端口 (Mixed Inbound Port)</span>
          <span class="sub-label">HTTP / SOCKS5 混合协议监听端口 (默认: 7890)</span>
        </div>
        <input
          type="number"
          v-model.number="settingsStore.settings.mixed_port"
          class="num-input"
          placeholder="7890"
          @change="save"
        />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>ClashAPI 控制端口 (ClashAPI External Controller Port)</span>
          <span class="sub-label">Sing-box ClashAPI 控制通信端口 (默认: 9090)</span>
        </div>
        <input
          type="number"
          v-model.number="settingsStore.settings.clash_api_port"
          class="num-input"
          placeholder="9090"
          @change="save"
        />
      </div>
      </div>
    </div>
  </template>

<script setup lang="ts">
import { useSettingsStore } from "@/stores/settings.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useToast } from "@/composables/useToast";

const settingsStore = useSettingsStore();
const proxyStore = useProxyStore();
const toast = useToast();

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("端口与代理模式设置已保存");
}

async function saveMode() {
  await proxyStore.changeProxyMode(settingsStore.settings.proxy_mode);
  await save();
}
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.select-input { padding: 6px 12px; background: #121622; border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; outline: none; }
.num-input { padding: 6px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; width: 100px; outline: none; }
.switch { width: 18px; height: 18px; cursor: pointer; }
</style>
