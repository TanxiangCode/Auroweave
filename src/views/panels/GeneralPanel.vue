<template>
  <div class="panel-container">
    <!-- 外观与偏好 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"></span>
        <div class="card-title-group">
          <h3>外观与语言</h3>
          <p>个性化界面视窗主题风格与显示语言</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>界面主题 (Theme)</span>
            <span class="sub-label">选择客户端视窗明暗极客风格</span>
          </div>
          <select v-model="settingsStore.settings.theme" class="select-input" @change="save">
            <option value="dark">深色极光 (Dark)</option>
            <option value="light">浅色明亮 (Light)</option>
            <option value="system">跟随操作系统 (System)</option>
          </select>
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>界面语言 (Language)</span>
            <span class="sub-label">切换客户端操作语言</span>
          </div>
          <select v-model="settingsStore.settings.language" class="select-input" @change="save">
            <option value="zh-CN">🇨🇳 简体中文 (Simplified Chinese)</option>
            <option value="en-US">🇺🇸 English (US)</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 视窗与启动行为 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"></span>
        <div class="card-title-group">
          <h3>系统托盘与自启行为</h3>
          <p>配置登录自启、托盘后台常驻与窗口关闭策略</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>登录开机自启动</span>
            <span class="sub-label">系统登录时自动拉起 Auroweave 内核服务</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.auto_start"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>关闭主窗口时最小化到托盘</span>
            <span class="sub-label">点击窗口右上角关闭按钮时常驻系统托盘，代理不中断</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.minimize_to_tray"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>关闭主窗口后隐藏程序坞 (Dock)</span>
            <span class="sub-label">窗口关闭后自动退出程序坞图标，转为极简托盘常驻模式 (macOS)</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.hide_dock_on_close"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>任务栏 / 托盘实时显示网速</span>
            <span class="sub-label">在系统顶部菜单栏或任务栏托盘中实时展示当前上下行吞吐速率 (↑ 0KB/s ↓ 0KB/s)</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.show_tray_speed"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>开机自启时静默启动</span>
            <span class="sub-label">开机自启时不弹出主界面，直接在后台静默运行</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.start_minimized"
            class="switch"
            @change="save"
          />
        </div>
      </div>
    </div>


    <!-- 网络共享与监控 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"></span>
        <div class="card-title-group">
          <h3>网络共享与应用审计</h3>
          <p>控制局域网内其他设备代理共享与应用级进程流量监控</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>允许局域网设备连接 (Allow LAN)</span>
            <span class="sub-label">允许同局域网的手机、平板或虚拟机通过本机 IP:{{ settingsStore.settings.mixed_port }} 连接代理</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.allow_lan"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>应用流量与审计追踪</span>
            <span class="sub-label">允许后台统计各个应用程序的流量消耗并进行安全审计</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.enable_app_traffic_tracking"
            class="switch"
            @change="save"
          />
        </div>
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
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.setting-card {
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 12px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  padding-bottom: 10px;
}

.card-icon {
  font-size: 20px;
}

.card-title-group h3 {
  font-size: 13.5px;
  font-weight: 700;
  color: #fff;
  margin: 0;
}

.card-title-group p {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  margin: 2px 0 0;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.02);
  transition: all 0.15s;
}

.setting-item:hover {
  background: rgba(255, 255, 255, 0.04);
}

.item-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12.5px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
}

.sub-label {
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.4);
  font-weight: normal;
}

.select-input {
  padding: 6px 12px;
  background: #141824;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  outline: none;
  cursor: pointer;
}

.select-input:focus {
  border-color: #00f2fe;
}

.switch {
  width: 18px;
  height: 18px;
  accent-color: #00f2fe;
  cursor: pointer;
}
</style>
