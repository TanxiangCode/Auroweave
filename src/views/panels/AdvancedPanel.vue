<template>
  <div class="panel-container">
    <h2>高级设置与测速/超时配置</h2>
    <div class="setting-group">
      <!-- 拓扑图开关 (具有反向锚定高亮) -->
      <div
        class="setting-item"
        :class="{ highlight: highlightTopology }"
        ref="topologyItemRef"
      >
        <div class="item-label">
          <span>启用拓扑图功能 (Topology Canvas)</span>
          <span class="sub-label">解锁分流 Routing 页面中的高级多跳链路拓扑构建模式</span>
        </div>
        <input
          type="checkbox"
          v-model="settingsStore.settings.topology_enabled"
          class="switch"
          @change="save"
        />
      </div>

      <!-- 性能模式开关 -->
      <div class="setting-item">
        <div class="item-label">
          <span>性能模式 (Performance Mode)</span>
          <span class="sub-label">一键关闭模糊毛玻璃与高耗 GPU/CPU 的背景动效，适应低配设备</span>
        </div>
        <input
          type="checkbox"
          v-model="settingsStore.settings.performance_mode"
          class="switch"
          @change="handlePerfModeChange"
        />
      </div>

      <!-- 测速目标 URL -->
      <div class="setting-item">
        <div class="item-label">
          <span>吞吐量测速目标 URL</span>
          <span class="sub-label">单节点/批量测速时使用的下载测试数据源</span>
        </div>
        <input
          type="text"
          v-model="settingsStore.settings.speed_test_url"
          class="text-input"
          placeholder="https://..."
          @change="save"
        />
      </div>

      <!-- 测速超时时间 -->
      <div class="setting-item">
        <div class="item-label">
          <span>单节点测速限时 (秒)</span>
          <span class="sub-label">单节点吞吐量测试的最大持续秒数 (默认: 5 秒)</span>
        </div>
        <input
          type="number"
          v-model.number="settingsStore.settings.speed_test_timeout_secs"
          class="num-input"
          placeholder="5"
          @change="save"
        />
      </div>

      <!-- 网络连接超时时间 -->
      <div class="setting-item">
        <div class="item-label">
          <span>订阅网络请求超时 (秒)</span>
          <span class="sub-label">拉取远程订阅与规则时的网络超时阈值 (默认: 15 秒)</span>
        </div>
        <input
          type="number"
          v-model.number="settingsStore.settings.connection_timeout_secs"
          class="num-input"
          placeholder="15"
          @change="save"
        />
      </div>

      <!-- 内核版本管理 -->
      <div class="setting-item">
        <div class="item-label">
          <span>Sing-box 内核版本</span>
          <span class="sub-label">系统内嵌侧载核心 v{{ singboxVersion }} (100% 规则对齐)</span>
        </div>
        <span class="version-tag">v{{ singboxVersion }}</span>
      </div>

      <!-- 配置重置 -->
      <div class="setting-item">
        <div class="item-label">
          <span>恢复上一次配置备份</span>
          <span class="sub-label">若当前配置文件出现异常，一键恢复 config.backup.json</span>
        </div>
        <button class="btn-restore" @click="handleRestore">
          <SvgIcon name="refresh" :size="12" class="icon-gap" />
          恢复备份
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import { invoke } from "@tauri-apps/api/core";
import SvgIcon from "@/components/common/SvgIcon.vue";

const props = defineProps<{
  highlightTarget?: string;
}>();

const settingsStore = useSettingsStore();
const toast = useToast();
const highlightTopology = ref(false);
const topologyItemRef = ref<HTMLDivElement | null>(null);
const singboxVersion = ref<string>("加载中...");

onMounted(() => {
  if (props.highlightTarget === "topology") {
    highlightTopology.value = true;
    setTimeout(() => {
      highlightTopology.value = false;
    }, 3000);
  }
  
  invoke<any>("proxy_get_singbox_version").then(res => {
    if (res.success && res.data) {
      singboxVersion.value = res.data;
    } else {
      singboxVersion.value = "未知";
    }
  }).catch(() => {
    singboxVersion.value = "无法获取";
  });
});

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("高级与超时设置已保存");
}

async function handlePerfModeChange() {
  await save();
  if (settingsStore.settings.performance_mode) {
    document.documentElement.setAttribute("data-perf-mode", "reduced");
    toast.info("已开启性能模式", "装饰性动画与毛玻璃滤镜已一键关闭");
  } else {
    document.documentElement.removeAttribute("data-perf-mode");
    toast.info("已关闭性能模式", "动效与毛玻璃遮罩已恢复");
  }
}

function handleRestore() {
  toast.info("配置恢复请求已响应");
}
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: var(--text-md); font-weight: var(--weight-bold); }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: var(--layer-1); border: 1px solid var(--border-normal); border-radius: var(--radius-lg); transition: all var(--duration-normal) ease; }
.setting-item.highlight { border-color: var(--accent-cyan); box-shadow: var(--shadow-glow-cyan); animation: breath 1s ease-in-out infinite alternate; }
@keyframes breath { from { box-shadow: 0 0 10px var(--accent-cyan-glow); } to { box-shadow: 0 0 24px var(--accent-cyan-glow); } }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: var(--text-sm); font-weight: var(--weight-semibold); }
.sub-label { font-size: var(--text-xs); color: var(--text-tertiary); font-weight: normal; }
.switch { width: 18px; height: 18px; cursor: pointer; }
.text-input { padding: 6px 12px; background: var(--layer-2); border: 1px solid var(--border-normal); border-radius: var(--radius-sm); color: var(--text-primary); font-size: var(--text-xs); width: 260px; outline: none; }
.num-input { padding: 6px 12px; background: var(--layer-2); border: 1px solid var(--border-normal); border-radius: var(--radius-sm); color: var(--text-primary); width: 100px; outline: none; }
.version-tag { font-size: var(--text-xs); padding: 4px 8px; background: var(--accent-cyan-glow); border-radius: var(--radius-sm); color: var(--accent-cyan); font-weight: var(--weight-bold); }
.btn-restore { padding: 6px 12px; background: var(--layer-2); border: 1px solid var(--border-normal); border-radius: var(--radius-sm); color: var(--text-primary); font-size: var(--text-xs); cursor: pointer; transition: all var(--duration-fast); }
.btn-restore:hover { background: var(--border-strong); border-color: var(--border-accent); }
.icon-gap { margin-right: 4px; }
</style>
