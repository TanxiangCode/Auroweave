<script setup lang="ts">
/**
 * TUN 网卡配置与系统服务管理面板 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定、子组件协调
 */
import { computed } from "vue";
import { useSettingsStore } from "@/stores/settings.store";

import RunModeSelector from "./components/RunModeSelector.vue";
import ServiceStatusPanel from "./components/ServiceStatusPanel.vue";
import TunInterfaceConfig from "./components/TunInterfaceConfig.vue";
import ServiceInstallModal from "./components/ServiceInstallModal.vue";
import ServiceLogModal from "./components/ServiceLogModal.vue";

import { useSystemService } from "./hooks/useSystemService";

const settingsStore = useSettingsStore();

const {
  serviceStatus,
  operating,
  errorMsg,
  showConfirmModal,
  showLogModal,
  serviceLog,
  runMode,
  statusText,
  selectMode,
  handleInstallService,
  handleStart,
  handleStop,
  handleUninstall,
  handleViewLog,
} = useSystemService();

/** TUN 网卡名称初始值 */
const tunInterfaceName = computed(
  () => settingsStore.settings.tun_interface_name || "Auroweave"
);
</script>

<template>
  <div class="panel-container">
    <h2>内核运行模式</h2>
    <p class="panel-desc">
      选择 GUI 主程序与 sing-box 内核的交互架构。在 Windows 下，推荐使用系统服务模式以实现免 UAC 的 TUN 网络接管体验。
    </p>

    <!-- 运行模式切换控制区 -->
    <RunModeSelector
      :run-mode="runMode"
      :operating="operating"
      @select="selectMode"
    />

    <!-- 提示直连模式开启 TUN 的小警告 -->
    <div
      v-if="runMode === 'local' && settingsStore.settings.tun_enabled"
      class="fallback-banner warning"
    >
      当前为本地运行模式，如果您尚未一键提权安装组件，启用 TUN 将提示提权。推荐切换至<strong>系统服务模式</strong>以获得流畅的开机自启体验。
    </div>

    <!-- 切换失败错误展示 -->
    <div v-if="errorMsg" class="fallback-banner error">
      操作失败: {{ errorMsg }}
    </div>

    <!-- 系统服务详细状态面板 -->
    <Transition name="fade-slide">
      <ServiceStatusPanel
        v-if="runMode === 'service'"
        :service-status="serviceStatus"
        :status-text="statusText"
        :operating="operating"
        @start="handleStart"
        @stop="handleStop"
        @view-log="handleViewLog"
        @uninstall="handleUninstall"
        @install="selectMode('service')"
      />
    </Transition>

    <hr class="separator" />

    <h2>TUN 虚拟网卡</h2>
    <p class="panel-desc">
      TUN 模式通过虚拟网卡在系统内核层接管流量，配置对应的参数选项：
    </p>

    <!-- TUN 网卡名称配置 -->
    <TunInterfaceConfig :initial-name="tunInterfaceName" />

    <!-- 为什么只需授权一次？科普卡片 -->
    <div class="info-card">
      <div class="info-icon"></div>
      <div class="info-body">
        <p><strong>关于免 UAC 系统服务工作机制：</strong></p>
        <p>
          Auroweave 采用 Windows 自定义安全描述符（DACL）机制。安装服务时会授予当前会话用户对该服务的启动与停止权限。
          配置为系统服务模式后，前端操作日常代理的开启/关闭，只需直接驱动后台服务启停而完全不需要系统弹出 UAC 管理员提权确认，既安全又优雅。
        </p>
      </div>
    </div>

    <!-- 弹窗区 -->
    <ServiceInstallModal
      :visible="showConfirmModal"
      @close="showConfirmModal = false"
      @confirm="handleInstallService"
    />

    <ServiceLogModal
      :visible="showLogModal"
      :log-content="serviceLog"
      @close="showLogModal = false"
      @refresh="handleViewLog"
    />
  </div>
</template>

<style scoped>
.panel-container h2 {
  font-size: var(--text-lg);
  font-weight: var(--weight-bold);
  margin: 0;
}

.panel-desc {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: var(--leading-relaxed);
  margin: 0;
}

/* 错误与回退横幅 */
.fallback-banner {
  padding: var(--space-3) var(--space-4);
  font-size: var(--text-xs);
  border-radius: var(--radius-sm);
  line-height: var(--leading-normal);
}

.fallback-banner.warning {
  background: var(--accent-orange-glow, rgba(249, 115, 22, 0.08));
  border: 1px solid var(--accent-orange, rgba(249, 115, 22, 0.25));
  color: var(--accent-orange);
}

.fallback-banner.warning strong {
  color: var(--accent-cyan-vivid);
}

.fallback-banner.error {
  background: var(--accent-red-glow);
  border: 1px solid var(--accent-red-glow);
  color: var(--accent-red);
}

/* 动效 */
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.3s ease;
}

.fade-slide-enter-from,
.fade-slide-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
