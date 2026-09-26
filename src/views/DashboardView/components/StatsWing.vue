<script setup lang="ts">
/**
 * 右翼统计区
 * 作者: TanXiang
 *
 * 展示活动连接数、累计流量、当前节点与延迟、出口 IP 归属地（国旗）、订阅到期
 * 点击可跳转，出口信息支持手动刷新
 */
import { computed } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { formatBytes } from "@/utils/format";
import { renderFlagSvg } from "@/utils/flag";
import type { EgressInfo } from "../hooks/useEgressInfo";

const props = defineProps<{
  activeConnectionCount: number;
  totalDownload: number;
  totalUpload: number;
  /** 当前工作节点名（含负载均衡/自动组递归解析后的物理节点） */
  currentNode?: string;
  /** 当前节点延迟（毫秒，0/undefined=未测） */
  currentNodeLatency?: number;
  /** 出口 IP 信息（含国家代码） */
  egressInfo: EgressInfo | null;
  /** 出口信息探测中 */
  egressLoading?: boolean;
  /** 各胶囊显隐开关（设置-首页显示；undefined 视为开启） */
  showConnections?: boolean;
  showCurrentNode?: boolean;
  showEgressIp?: boolean;
  showTotalTraffic?: boolean;
}>();

const emit = defineEmits<{
  navigate: [path: string];
  refreshEgress: [];
}>();

/** 国旗 SVG（未收录代码也有兜底徽章） */
const flagSvg = computed(() => {
  const cc = props.egressInfo?.countryCode;
  return cc ? renderFlagSvg(cc) : "";
});

/** 延迟显示色（沿用 NodeCard 同款阈值语义：绿/黄/红） */
const latencyColor = computed(() => {
  const ms = props.currentNodeLatency;
  if (!ms || ms <= 0) return "var(--text-tertiary)";
  if (ms < 150) return "var(--accent-green)";
  if (ms < 400) return "var(--status-warning)";
  return "var(--accent-red)";
});
</script>

<template>
  <div class="stats-wing">
    <!-- 胶囊 1: 活动连接 -->
    <div
      v-if="showConnections !== false"
      class="stat-pill clickable-pill"
      @click="emit('navigate', '/audit')"
      title="点击查看实时连接审计"
    >
      <span class="pill-label">活动连接</span>
      <span class="pill-val cyan-glow">{{ activeConnectionCount }} 条</span>
    </div>

    <!-- 胶囊 2: 当前节点 + 延迟 -->
    <div
      v-if="showCurrentNode !== false"
      class="stat-pill clickable-pill"
      @click="emit('navigate', '/proxies')"
      title="点击切换代理节点"
    >
      <span class="pill-label">当前节点</span>
      <span class="pill-val node-val">
        <span class="node-name" :title="currentNode">{{ currentNode || "未选择" }}</span>
        <span
          v-if="currentNodeLatency && currentNodeLatency > 0"
          class="latency-chip"
          :style="{ color: latencyColor }"
        >{{ currentNodeLatency }} ms</span>
        <span v-else class="latency-chip untested">未测</span>
      </span>
    </div>

    <!-- 胶囊 3: 出口 IP + 归属地（右侧国旗） -->
    <div
      v-if="showEgressIp !== false"
      class="stat-pill clickable-pill"
      @click="emit('refreshEgress')"
      title="点击重新探测出口 IP（当前流量真实出口）"
    >
      <span class="pill-label">
        <BaseIcon v-if="egressLoading" name="RefreshCw" :size="11" class="spin-icon" />
        出口 IP
      </span>
      <span class="pill-val ip-val">
        <template v-if="egressInfo && egressInfo.ok">
          <span class="ip-text">{{ egressInfo.ip }}</span>
          <span class="ip-loc" :title="egressInfo.location">{{ egressInfo.location }}</span>
          <span v-if="flagSvg" class="flag-badge" v-html="flagSvg"></span>
        </template>
        <span v-else-if="egressLoading" class="ip-text muted">探测中...</span>
        <span v-else class="ip-text muted">点击探测</span>
      </span>
    </div>

    <!-- 胶囊 4: 累计流量 -->
    <div
      v-if="showTotalTraffic !== false"
      class="stat-pill clickable-pill"
      @click="emit('navigate', '/stats')"
      title="点击查看详细流量统计"
    >
      <span class="pill-label">累计流量</span>
      <span class="pill-val data-combined-val">
        <span class="down-flow">↓ {{ formatBytes(totalDownload) }}</span>
        <span class="divider">|</span>
        <span class="up-flow">↑ {{ formatBytes(totalUpload) }}</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.stats-wing {
  display: flex;
  flex-direction: column;
  gap: 12px;
  justify-content: flex-end;
  height: 100%;
  padding-bottom: 10px;
}

.stat-pill {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 20px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  min-height: 42px;
  transition: all var(--duration-fast) var(--ease-out);
}

.stat-pill.clickable-pill {
  cursor: pointer;
}

.stat-pill.clickable-pill:hover {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  box-shadow: 0 0 12px var(--accent-blue-glow);
  transform: translateX(-4px);
}

.pill-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-bold);
  letter-spacing: 0.5px;
  white-space: nowrap;
}

.pill-val {
  font-size: var(--text-xs);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.pill-val.cyan-glow {
  color: var(--accent-cyan);
  text-shadow: 0 0 6px var(--accent-cyan-glow);
}

/* 当前节点胶囊 */
.node-val {
  justify-content: flex-end;
  gap: 8px;
  max-width: 150px;
}

.node-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
}

.latency-chip {
  font-size: 10px;
  font-weight: var(--weight-bold);
  white-space: nowrap;
}

.latency-chip.untested {
  color: var(--text-tertiary);
  font-weight: var(--weight-regular);
}

/* 出口 IP 胶囊 */
.ip-val {
  flex-direction: column;
  align-items: flex-end;
  gap: 0;
  position: relative;
  padding-right: 26px;
}

.ip-text {
  font-size: 11px;
  white-space: nowrap;
}

.ip-text.muted {
  color: var(--text-tertiary);
  font-weight: var(--weight-regular);
}

.ip-loc {
  font-size: 10px;
  color: var(--text-tertiary);
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-sans, sans-serif);
}

.flag-badge {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  width: 18px;
  height: 13.5px;
  border-radius: 2px;
  overflow: hidden;
  flex-shrink: 0;
  border: 1px solid var(--border-strong);
  line-height: 0;
}

.flag-badge :deep(.flag-svg) {
  width: 100%;
  height: 100%;
  display: block;
}

.spin-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.data-combined-val {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.down-flow {
  color: rgb(0, 127, 249);
}

.up-flow {
  color: rgb(254, 49, 56);
}

.divider {
  color: var(--border-strong);
  font-weight: var(--weight-light);
  opacity: 0.5;
}
</style>
