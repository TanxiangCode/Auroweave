<template>
  <div class="panel-container">
    <!-- 1. Sing-box 内核版本与在线升级管理 -->
    <div class="setting-card glass-effect highlight-border">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="FlaskConical" :size="20" /></span>
        <div class="card-title-group">
          <h3>Sing-box 内核版本管理</h3>
          <p>检测官方 GitHub Release 最新内核版本并提供一键在线升级与热重启</p>
        </div>
        <button class="btn-check-update" @click="handleCheckUpdate" :disabled="checkingUpdate || upgrading">
          <span :class="{ spinning: checkingUpdate }"></span>
          <span>{{ checkingUpdate ? '正在检查...' : '检查更新' }}</span>
        </button>
      </div>

      <div class="card-body">
        <div class="kernel-status-row">
          <div class="status-left">
            <span class="status-label">当前运行内核：</span>
            <span class="version-badge current">v{{ currentVersion }}</span>
            <span class="status-text">100% 规则对齐 (Official Sidecar)</span>
          </div>

          <div class="status-right" v-if="updateInfo">
            <span v-if="updateInfo.has_update" class="update-found-badge">
              发现新版本 {{ updateInfo.latest_version }}
            </span>
            <span v-else class="up-to-date-badge"><BaseIcon name="Check" :size="13" /> 已是最新版本</span>
          </div>
        </div>

        <!-- 升级通知卡片（发现新版本时显示） -->
        <div v-if="updateInfo && updateInfo.has_update" class="update-release-box">
          <div class="release-header">
            <div class="release-title">
              <span class="release-tag">{{ updateInfo.latest_version }}</span>
              <span class="release-date">发布于 {{ formatDate(updateInfo.published_at) }}</span>
            </div>
            <button
              class="btn-upgrade-now"
              @click="handleUpgrade"
              :disabled="upgrading"
            >
              <span v-if="!upgrading">立即在线升级</span>
              <span v-else class="upgrading-state">
                <span class="spinner"></span>
                正在下载并替换内核 (请勿关闭)...
              </span>
            </button>
          </div>

          <!-- 更新日志摘要 -->
          <div class="release-notes" v-if="updateInfo.release_notes">
            <div class="notes-title">更新日志 (Changelog):</div>
            <pre class="notes-content">{{ updateInfo.release_notes }}</pre>
          </div>
        </div>
      </div>
    </div>

    <!-- 2. 延迟与吞吐量测速配置 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="FlaskConical" :size="20" /></span>
        <div class="card-title-group">
          <h3>延迟测试与吞吐量测速</h3>
          <p>配置全节点批量并发测速、下载吞吐量测试源与网络超时阈值</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试目标 URL</span>
            <span class="sub-label">节点延迟连通性测试的探测端点 (返回 HTTP 204)</span>
          </div>
          <input
            type="text"
            v-model="settingsStore.settings.latency_test_url"
            class="text-input"
            placeholder="http://www.gstatic.com/generate_204"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试并发数</span>
            <span class="sub-label">一键全量测速时允许的最大并发请求通道数 (推荐: 20~50)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.latency_test_concurrency"
            class="num-input"
            min="1"
            max="100"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>延迟测试超时 (毫秒)</span>
            <span class="sub-label">单节点 TCP/TLS 握手与连通性超时阈值 (默认: 3000 ms)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.latency_test_timeout_ms"
            class="num-input"
            step="500"
            min="1000"
            max="10000"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>吞吐量下载测速 URL</span>
            <span class="sub-label">节点速度测试时的流式下载数据源</span>
          </div>
          <input
            type="text"
            v-model="settingsStore.settings.speed_test_url"
            class="text-input"
            placeholder="https://speed.cloudflare.com/__down?bytes=25000000"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>单节点下载测速限时 (秒)</span>
            <span class="sub-label">吞吐量下载速度测试的最大持续时间 (默认: 5 秒)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.speed_test_timeout_secs"
            class="num-input"
            min="3"
            max="30"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>订阅网络请求超时 (秒)</span>
            <span class="sub-label">远程拉取订阅节点与规则集的连接超时时长 (默认: 15 秒)</span>
          </div>
          <input
            type="number"
            v-model.number="settingsStore.settings.connection_timeout_secs"
            class="num-input"
            min="5"
            max="60"
            @change="save"
          />
        </div>
      </div>
    </div>

    <!-- 3. 系统性能与灾备恢复 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="FlaskConical" :size="20" /></span>
        <div class="card-title-group">
          <h3>性能模式与灾备恢复</h3>
          <p>控制 GPU 毛玻璃渲染与异常配置一键还原</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item" :class="{ highlight: highlightTopology }">
          <div class="item-label">
            <span>启用拓扑网络图 (Topology Canvas)</span>
            <span class="sub-label">在分流页面启用全景 4 层动态交互式贝塞尔流光拓扑画布</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.topology_enabled"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>性能低耗模式 (Performance Mode)</span>
            <span class="sub-label">一键关闭背景毛玻璃与高耗 GPU/CPU 动画滤镜，适应低功耗场景</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.performance_mode"
            class="switch"
            @change="handlePerfModeChange"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>恢复上一次配置备份</span>
            <span class="sub-label">若当前内核配置文件异常，一键恢复 config.backup.json 并重启</span>
          </div>
          <button class="btn-restore" @click="handleRestore" :disabled="restoring">
            <span>{{ restoring ? '正在恢复...' : '恢复备份' }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, onMounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import { invoke } from "@tauri-apps/api/core";
import {
  checkSingboxUpdate,
  upgradeSingbox,
  restoreConfigBackup,
} from "@/api/ipc/settings";
import type { SingboxUpdateInfo } from "@/types";

const props = defineProps<{
  highlightTarget?: string;
}>();

const settingsStore = useSettingsStore();
const toast = useToast();

const currentVersion = ref<string>("加载中...");
const highlightTopology = ref(false);
const checkingUpdate = ref(false);
const upgrading = ref(false);
const restoring = ref(false);
const updateInfo = ref<SingboxUpdateInfo | null>(null);

function formatDate(dateStr: string): string {
  if (!dateStr) return "";
  try {
    const d = new Date(dateStr);
    return d.toLocaleDateString("zh-CN", { year: "numeric", month: "long", day: "numeric" });
  } catch {
    return dateStr;
  }
}

async function fetchCurrentVersion() {
  try {
    const res = await invoke<any>("proxy_get_singbox_version");
    if (res.success && res.data) {
      currentVersion.value = res.data;
    } else {
      currentVersion.value = "未知";
    }
  } catch {
    currentVersion.value = "无法获取";
  }
}

async function handleCheckUpdate() {
  checkingUpdate.value = true;
  toast.info("正在查询 GitHub Release 最新版本...");
  const res = await checkSingboxUpdate();
  checkingUpdate.value = false;

  if (res.success && res.data) {
    updateInfo.value = res.data;
    if (res.data.has_update) {
      toast.info(`发现新版本 ${res.data.latest_version}`, "点击「立即升级」可一键自动更新内核");
    } else {
      toast.success("当前已是最新内核版本", `v${res.data.current_version}`);
    }
  } else {
    toast.error("检查更新失败", res.error || "无法连接到 GitHub API");
  }
}

async function handleUpgrade() {
  if (!updateInfo.value || !updateInfo.value.download_url) {
    toast.error("未找到对应平台的下载资产");
    return;
  }

  upgrading.value = true;
  toast.info("正在下载内核安装包并执行热替换，请稍候...");
  const res = await upgradeSingbox(updateInfo.value.download_url);
  upgrading.value = false;

  if (res.success) {
    toast.success("Sing-box 内核升级成功！", "新版本已自动替换并重新拉起运行");
    await fetchCurrentVersion();
    updateInfo.value.has_update = false;
  } else {
    toast.error("内核升级失败", res.error || "下载或解压过程中发生异常");
  }
}

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("高级设置已保存");
}

async function handlePerfModeChange() {
  await save();
  if (settingsStore.settings.performance_mode) {
    document.documentElement.setAttribute("data-perf-mode", "reduced");
    toast.info("已开启性能模式", "装饰性动画与毛玻璃滤镜已一键关闭");
  } else {
    document.documentElement.removeAttribute("data-perf-mode");
    toast.info("已关闭性能模式", "动效与毛玻璃滤镜已恢复");
  }
}

async function handleRestore() {
  if (!confirm("确定要恢复上次的配置备份 (config.backup.json) 并重启核心吗？")) {
    return;
  }
  restoring.value = true;
  toast.info("正在恢复配置文件并重启核心...");
  const res = await restoreConfigBackup();
  restoring.value = false;
  if (res.success) {
    toast.success("配置备份已成功恢复", "内核已重新加载运行");
  } else {
    toast.error("恢复备份失败", res.error);
  }
}

onMounted(() => {
  if (props.highlightTarget === "topology") {
    highlightTopology.value = true;
    setTimeout(() => {
      highlightTopology.value = false;
    }, 3000);
  }
  fetchCurrentVersion();
});
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

.setting-card.highlight-border {
  border-color: rgba(0, 242, 254, 0.25);
  background: linear-gradient(180deg, rgba(0, 242, 254, 0.03) 0%, rgba(255, 255, 255, 0.02) 100%);
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

.card-title-group {
  flex: 1;
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

.btn-check-update {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: rgba(0, 242, 254, 0.12);
  border: 1px solid rgba(0, 242, 254, 0.3);
  border-radius: 8px;
  color: #00f2fe;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-check-update:hover:not(:disabled) {
  background: #00f2fe;
  color: #000;
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.kernel-status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.02);
  border-radius: 8px;
}

.status-left {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
}

.status-label {
  color: rgba(255, 255, 255, 0.6);
}

.version-badge.current {
  padding: 2px 8px;
  background: rgba(0, 242, 254, 0.15);
  border: 1px solid rgba(0, 242, 254, 0.35);
  border-radius: 6px;
  color: #00f2fe;
  font-weight: 700;
  font-family: monospace;
}

.status-text {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.35);
}

.update-found-badge {
  padding: 3px 8px;
  background: rgba(239, 68, 68, 0.15);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: 6px;
  color: #f87171;
  font-size: 11.5px;
  font-weight: 600;
}

.up-to-date-badge {
  color: #10b981;
  font-size: 12px;
  font-weight: 600;
}

.update-release-box {
  background: rgba(0, 242, 254, 0.04);
  border: 1px dashed rgba(0, 242, 254, 0.3);
  border-radius: 10px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.release-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.release-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.release-tag {
  font-size: 14px;
  font-weight: 700;
  color: #fff;
  font-family: monospace;
}

.release-date {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.btn-upgrade-now {
  padding: 7px 16px;
  background: #00f2fe;
  color: #000;
  border: none;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-upgrade-now:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 0 15px rgba(0, 242, 254, 0.4);
}

.btn-upgrade-now:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.upgrading-state {
  display: flex;
  align-items: center;
  gap: 6px;
}

.release-notes {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.notes-title {
  font-size: 11.5px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.7);
}

.notes-content {
  font-size: 11px;
  font-family: monospace;
  color: rgba(255, 255, 255, 0.6);
  background: rgba(0, 0, 0, 0.3);
  padding: 8px 10px;
  border-radius: 6px;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  margin: 0;
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

.text-input {
  padding: 6px 10px;
  background: #141824;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  width: 260px;
  outline: none;
  font-family: monospace;
}

.text-input:focus,
.num-input:focus {
  border-color: #00f2fe;
}

.num-input {
  padding: 6px 10px;
  background: #141824;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  width: 90px;
  outline: none;
}

.btn-restore {
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
}

.btn-restore:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.1);
}

.switch {
  width: 18px;
  height: 18px;
  accent-color: #00f2fe;
  cursor: pointer;
}
</style>
