<template>
  <div class="panel-container">
    <h2>⚙️ 通用设置</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>外观主题</span>
          <span class="sub-label">选择客户端视窗与视觉风格</span>
        </div>
        <select v-model="settingsStore.settings.theme" class="select-input" @change="save">
          <option value="dark">🌙 深色极客 (Dark)</option>
          <option value="light">☀️ 浅色明亮 (Light)</option>
          <option value="system">💻 跟随系统 (System)</option>
        </select>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>应用语言</span>
          <span class="sub-label">切换界面语言</span>
        </div>
        <select v-model="settingsStore.settings.language" class="select-input" @change="save">
          <option value="zh-CN">🇨🇳 简体中文</option>
          <option value="en-US">🇺🇸 English</option>
        </select>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>开机自启动</span>
          <span class="sub-label">系统登录时自动拉起 Auroweave 后台服务</span>
        </div>
        <input type="checkbox" v-model="settingsStore.settings.auto_start" class="switch" @change="save" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";

const settingsStore = useSettingsStore();
const toast = useToast();

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("通用设置已保存");
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
.switch { width: 18px; height: 18px; cursor: pointer; }
</style>
