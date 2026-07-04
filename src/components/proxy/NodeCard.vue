<template>
  <div
    class="node-card"
    :class="{ active: isActive, testing: isTesting }"
    @click="$emit('select', nodeTag)"
  >
    <div class="node-header">
      <span class="node-tag" :title="nodeTag">{{ nodeTag }}</span>
      <span class="node-type">{{ nodeType }}</span>
    </div>

    <div class="node-metrics">
      <!-- 延迟展示与单独测速按钮 -->
      <button class="badge latency" :style="{ color: latencyColor }" @click.stop="$emit('test-latency', nodeTag)">
        ⚡ {{ latency ? `${latency} ms` : '测延迟' }}
      </button>

      <!-- 吞吐量测速按钮 -->
      <button class="badge throughput" :class="{ testing: isTesting }" @click.stop="$emit('test-speed', nodeTag)">
        <span v-if="isTesting" class="spinner">🌀</span>
        <span v-else-if="speedBps !== undefined && speedBps > 0">📶 {{ formatSpeed(speedBps) }}</span>
        <span v-else>🚀 测速</span>
      </button>
    </div>

    <div class="node-footer">
      <span class="status-indicator" :class="{ active: isActive }">
        {{ isActive ? '● 当前选中' : '○ 点击切换' }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";

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

const isTesting = computed(() => speedtestStore.testingNodes.has(props.nodeTag));
const latencyColor = computed(() => speedtestStore.getLatencyColor(props.latency));

function formatSpeed(bps: number): string {
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
}
</script>

<style scoped>
.node-card {
  padding: 14px 16px;
  background: rgba(255, 255, 255, 0.03);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-lg, 12px);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 12px;
  transition: all 0.2s ease;
}

.node-card:hover {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.18);
  transform: translateY(-2px);
}

.node-card.active {
  border-color: #00f2fe;
  background: rgba(0, 242, 254, 0.08);
  box-shadow: 0 0 16px rgba(0, 242, 254, 0.2);
}

.node-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.node-tag {
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-type {
  font-size: 10px;
  padding: 2px 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.5);
  text-transform: uppercase;
}

.node-metrics {
  display: flex;
  gap: 8px;
}

.badge {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 5px 8px;
  font-size: 11px;
  font-weight: 600;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.8);
  cursor: pointer;
  transition: all 0.15s ease;
}

.badge:hover {
  background: rgba(255, 255, 255, 0.12);
}

.badge.throughput.testing {
  color: #00f2fe;
  border-color: #00f2fe;
}

.spinner {
  display: inline-block;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.node-footer {
  display: flex;
  justify-content: flex-end;
}

.status-indicator {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.status-indicator.active {
  color: #00f2fe;
  font-weight: 600;
}
</style>
