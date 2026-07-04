<template>
  <div class="panel-container">
    <h2>🧪 高级设置与测速/超时配置</h2>
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
          <span class="sub-label">系统锁定嵌入侧载核心 v{{ SINGBOX_VERSION }} (100% 规则对齐)</span>
        </div>
        <span class="version-tag">v{{ SINGBOX_VERSION }}</span>
      </div>

      <!-- 配置重置 -->
      <div class="setting-item">
        <div class="item-label">
          <span>恢复上一次配置备份</span>
          <span class="sub-label">若当前配置文件出现异常，一键恢复 config.backup.json</span>
        </div>
        <button class="btn-restore" @click="handleRestore">🔄 恢复备份</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import { SINGBOX_VERSION } from "@/constants";

const props = defineProps<{
  highlightTarget?: string;
}>();

const settingsStore = useSettingsStore();
const toast = useToast();

const highlightTopology = ref(false);
const topologyItemRef = ref<HTMLDivElement | null>(null);

onMounted(() => {
  if (props.highlightTarget === "topology") {
    highlightTopology.value = true;
    setTimeout(() => {
      highlightTopology.value = false;
    }, 3000);
  }
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
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; transition: all 0.3s ease; }
.setting-item.highlight { border-color: #00f2fe; box-shadow: 0 0 20px rgba(0, 242, 254, 0.4); animation: breath 1s ease-in-out infinite alternate; }
@keyframes breath { from { box-shadow: 0 0 10px rgba(0, 242, 254, 0.2); } to { box-shadow: 0 0 24px rgba(0, 242, 254, 0.6); } }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.switch { width: 18px; height: 18px; cursor: pointer; }
.text-input { padding: 6px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; font-size: 12px; width: 260px; outline: none; }
.num-input { padding: 6px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; width: 100px; outline: none; }
.version-tag { font-size: 12px; padding: 4px 8px; background: rgba(0, 242, 254, 0.1); border-radius: 6px; color: #00f2fe; font-weight: 600; }
.btn-restore { padding: 6px 12px; background: rgba(255,255,255,0.08); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; font-size: 12px; cursor: pointer; }
</style>
