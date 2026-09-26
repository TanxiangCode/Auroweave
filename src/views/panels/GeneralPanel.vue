<script setup lang="ts">
import { useSettingsStore } from "@/stores/settings.store";
import type { AppSettings } from "@/types";
import BaseIcon from "@/components/common/BaseIcon.vue";

const settingsStore = useSettingsStore();

/**
 * 局部 patch 保存：仅提交当前修改的字段，避免全量 settings
 * 覆盖其他面板尚未保存的修改或重复触发不必要的内核重启。
 * @param field - 本次变更的设置键名
 */
async function save(field: keyof AppSettings) {
  await settingsStore.updateSettings({ [field]: settingsStore.settings[field] } as Partial<AppSettings>);
}

</script>

<template>
  <div class="panel-container">
    <!-- 外观与偏好 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon">
          <BaseIcon name="Palette" :size="20" />
        </span>
        <div class="card-title-group">
          <h3>外观</h3>
          <p>个性化界面视窗主题风格</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>界面主题 (Theme)</span>
            <span class="sub-label">选择客户端视窗明暗极客风格</span>
          </div>
          <select v-model="settingsStore.settings.theme" class="select-input" @change="save('theme')">
            <option value="dark">深色极光 (Dark)</option>
            <option value="light">浅色明亮 (Light)</option>
            <option value="system">跟随操作系统 (System)</option>
          </select>
        </div>
      </div>
    </div>

    <!-- 视窗与启动行为 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon">
          <BaseIcon name="Sliders" :size="20" />
        </span>
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
            @change="save('auto_start')"
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
            @change="save('minimize_to_tray')"
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
            @change="save('hide_dock_on_close')"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>任务栏 / 托盘实时显示网速</span>
            <span class="sub-label">在系统顶部菜单栏中以双排定宽布局展示上下行速率（上行 ↑ 1.2M，下行 ↓ 12.3M）</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.show_tray_speed"
            class="switch"
            @change="save('show_tray_speed')"
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
            @change="save('start_minimized')"
          />
        </div>
      </div>
    </div>


    <!-- 网络共享与监控 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon">
          <BaseIcon name="Network" :size="20" />
        </span>
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
            @change="save('allow_lan')"
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
            @change="save('enable_app_traffic_tracking')"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 面板骨架（.panel-container / .setting-card / .card-* / .setting-item /
   .item-label / .sub-label / .panel-header-icon）统一走 panel.css 全局定义，
   此处只保留本面板独有的控件样式。 */

/* switch 统一走 App.vue 全局胶囊开关（36×20，勾选青色高亮） */
/* 输入框 / 下拉统一走 common.css 全局控件体系 */
</style>
