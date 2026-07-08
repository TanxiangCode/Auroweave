<template>
  <div
    class="node-row"
    :class="{ active: isActive, testing: isTesting }"
    @click="$emit('select', nodeTag)"
  >
    <!-- 左侧: 协议徽章与节点名称 -->
    <div class="node-left">
      <span class="protocol-badge" :title="nodeType">{{ getProtocolBadge(nodeType) }}</span>
      <div class="node-info">
        <span class="node-name" :title="nodeTag">{{ nodeTag }}</span>
        <span class="node-type">{{ nodeType }}</span>
      </div>
    </div>

    <!-- 中间: 延迟与吞吐量数值 -->
    <div class="node-middle">
      <span class="latency-indicator" :style="{ color: latencyColor }">
        <span class="status-dot" :style="{ backgroundColor: latencyColor }"></span>
        {{ latency ? `${latency} ms` : '未测试' }}
      </span>
      <span v-if="speedBps !== undefined && speedBps > 0" class="speed-val">
        <SvgIcon name="wifi" :size="10" style="margin-right: 2px;" />
        {{ formatSpeed(speedBps) }}
      </span>
    </div>

    <!-- 右侧: 操作按钮与激活状态 -->
    <div class="node-right">
      <button
        class="action-btn btn-ping"
        :class="{ testing: isLatencyTesting }"
        :disabled="isLatencyTesting"
        title="测试延迟"
        @click.stop="$emit('test-latency', nodeTag)"
      >
        <span v-if="isLatencyTesting" class="spinner">🌀</span>
        <SvgIcon v-else name="bolt" :size="12" />
      </button>
      <button
        class="action-btn btn-speed"
        :class="{ testing: isTesting }"
        :disabled="isTesting"
        title="吞吐量测速"
        @click.stop="$emit('test-speed', nodeTag)"
      >
        <span v-if="isTesting" class="spinner">🌀</span>
        <SvgIcon v-else name="wifi" :size="12" />
      </button>
      <div class="select-indicator">
        <SvgIcon v-if="isActive" name="check" :size="12" class="check-mark" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 策略组节点行组件 (替换 Emoji 为精美 UI 徽章与 SvgIcon)
 * 作者: TanXiang
 */
import { computed } from "vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import SvgIcon from "@/components/common/SvgIcon.vue";

const props = defineProps<{
  nodeTag: string;
  nodeType: string;
  isActive?: boolean;
  latency?: number;
  speedBps?: number;
}>();

defineEmits<{
  (e: "select", tag: string): void;
  (e: "test-latency", tag: string): void;
  (e: "test-speed", tag: string): void;
}>();

const speedtestStore = useSpeedtestStore();

function getProtocolBadge(type: string): string {
  const t = type.toLowerCase();
  if (t.includes("vmess")) return "VM";
  if (t.includes("vless")) return "VL";
  if (t.includes("trojan")) return "TJ";
  if (t.includes("shadowsocks") || t.includes("ss")) return "SS";
  if (t.includes("hysteria")) return "HY";
  if (t.includes("tuic")) return "TC";
  if (t.includes("wireguard")) return "WG";
  if (t.includes("direct")) return "DR";
  if (t.includes("block")) return "BL";
  if (t.includes("dns")) return "DS";
  return "PR";
}

function getLatencyColor(ms?: number): string {
  if (!ms || ms <= 0) return "var(--text-tertiary)";
  if (ms < 100) return "var(--accent-green)";
  if (ms < 300) return "var(--accent-orange)";
  return "var(--accent-red)";
}

const isTesting = computed(() => speedtestStore.testingNodes.has(props.nodeTag));
const isLatencyTesting = computed(() => speedtestStore.testingLatencyNodes.has(props.nodeTag));
const latencyColor = computed(() => getLatencyColor(props.latency));

function formatSpeed(bps: number): string {
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
}
</script>

<style scoped>
.node-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
  gap: 16px;
}

.node-row:hover {
  background: var(--layer-2);
  border-color: var(--border-normal);
  transform: translateX(2px);
}

.node-row.active {
  border-color: var(--accent-blue);
  background: var(--accent-blue-glow);
}

.node-left {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1;
}

.protocol-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 18px;
  font-size: 10px;
  font-weight: var(--weight-bold);
  border-radius: var(--radius-xs);
  background: var(--layer-3);
  color: var(--text-secondary);
  border: 1px solid var(--border-subtle);
  font-family: var(--font-mono);
  flex-shrink: 0;
}

.node-row.active .protocol-badge {
  background: var(--accent-blue);
  color: var(--text-on-accent);
  border-color: transparent;
}

.node-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.node-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-type {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-transform: uppercase;
  font-family: var(--font-mono);
  display: none; /* 已用协议徽章，这里可省去空间 */
}

.node-middle {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-shrink: 0;
}

.latency-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.speed-val {
  display: inline-flex;
  align-items: center;
  font-size: var(--text-xs);
  color: var(--accent-cyan);
  font-weight: var(--weight-semibold);
  font-family: var(--font-mono);
}

.node-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-xs);
  border: 1px solid var(--border-subtle);
  background: rgba(255, 255, 255, 0.02);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.action-btn:hover {
  background: var(--border-subtle);
  color: var(--text-primary);
  border-color: var(--border-normal);
}

.action-btn.btn-ping:hover {
  color: var(--accent-orange);
}

.action-btn.btn-ping.testing {
  color: var(--accent-orange);
  border-color: var(--accent-orange-glow);
}

.action-btn.btn-speed.testing {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan-glow);
}

.action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.spinner {
  display: inline-block;
  animation: spin 1s linear infinite;
  font-size: 11px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.select-indicator {
  width: 16px;
  display: flex;
  justify-content: center;
  align-items: center;
}

.check-mark {
  color: var(--accent-blue);
}
</style>
