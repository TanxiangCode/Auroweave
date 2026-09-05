<template>
  <div
    class="node-card"
    :class="[
      layoutMode,
      {
        active: isActive,
        testing: isTesting || isLatencyTesting,
        'non-selectable': !isSelectable
      }
    ]"
    @click="onCardClick"
  >
    <!-- 头部/左侧: 协议标签、节点名称、选中标识 -->
    <div class="node-main-info">
      <div class="node-badge-title">
        <span class="protocol-badge" :title="nodeType">{{ getProtocolBadge(nodeType) }}</span>
        <span class="node-title" :title="nodeTag">{{ nodeTag }}</span>
      </div>

      <!-- 选择指示器 -->
      <div class="select-badge-wrapper">
        <template v-if="isActive">
          <div v-if="isSelectable" class="active-check-circle" title="当前选中出站节点">
            <SvgIcon name="check" :size="11" />
          </div>
          <span v-else class="auto-active-badge" title="当前自动测速优选出口">自动优选</span>
        </template>
      </div>
    </div>

    <!-- 底部/右侧: 延迟、带宽与测速操作 -->
    <div class="node-metrics-bar">
      <div class="metrics-left">
        <!-- 延迟指示器 -->
        <div class="latency-box" :style="{ color: latencyColor }">
          <span class="latency-dot" :style="{ backgroundColor: latencyColor }"></span>
          <span class="latency-text">
            {{ latency === undefined ? '未测' : (latency === -1 || latency === 0 ? '超时' : `${latency}ms`) }}
          </span>
        </div>

        <!-- 吞吐量带宽 -->
        <div v-if="speedBps !== undefined && speedBps > 0" class="speed-box" title="历史下行测速结果">
          <SvgIcon name="wifi" :size="10" />
          <span>{{ formatThroughputCompact(speedBps) }}</span>
        </div>
      </div>

      <!-- 快捷操作按钮组 -->
      <div class="card-actions">
        <!-- 置顶收藏星标 -->
        <button
          class="card-action-btn pin-btn"
          :class="{ pinned: isPinned }"
          :title="isPinned ? '取消置顶收藏' : '置顶收藏（排序时恒排最前）'"
          @click.stop="$emit('toggle-pin', nodeTag)"
        >
          <BaseIcon name="Star" :size="12" />
        </button>

        <button
          class="card-action-btn ping-btn"
          :class="{ active: isLatencyTesting }"
          :disabled="isLatencyTesting"
          title="单个节点延迟测试"
          @click.stop="$emit('test-latency', nodeTag)"
        >
          <span v-if="isLatencyTesting" class="spin-icon"><BaseIcon name="RefreshCw" :size="12" class="spin" /></span>
          <SvgIcon v-else name="bolt" :size="11" />
        </button>

        <button
          class="card-action-btn speed-btn"
          :class="{ active: isTesting }"
          :disabled="isTesting"
          title="单个节点下行测速"
          @click.stop="$emit('test-speed', nodeTag)"
        >
          <span v-if="isTesting" class="spin-icon"><BaseIcon name="RefreshCw" :size="12" class="spin" /></span>
          <SvgIcon v-else name="wifi" :size="11" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 策略组节点卡片组件 (支持 Grid / List 视图)
 * 作者: TanXiang
 */
import { computed } from "vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import BaseIcon from "@/components/common/BaseIcon.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";
// 测速结果为字节/秒（字段名 download_bps 为历史误称），按 1024 进制 KB/s 展示
import { formatThroughputCompact } from "@/utils/format";

const props = withDefaults(
  defineProps<{
    nodeTag: string;
    nodeType: string;
    isActive?: boolean;
    latency?: number;
    speedBps?: number;
    isSelectable?: boolean;
    layoutMode?: "grid" | "list";
    /** 是否置顶收藏（星标高亮） */
    isPinned?: boolean;
  }>(),
  {
    isActive: false,
    isSelectable: true,
    layoutMode: "grid",
    isPinned: false,
  }
);

const emit = defineEmits<{
  (e: "select", tag: string): void;
  (e: "test-latency", tag: string): void;
  (e: "test-speed", tag: string): void;
  (e: "toggle-pin", tag: string): void;
}>();

function onCardClick() {
  if (props.isSelectable) {
    emit("select", props.nodeTag);
  }
}

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
  if (ms === undefined) return "var(--text-tertiary)"; // 未测试: 灰色
  if (ms === -1 || ms === 0) return "var(--accent-red)"; // 超时/错误: 红色
  if (ms < 100) return "var(--accent-green)"; // < 100ms: 极速 (翠绿)
  if (ms < 200) return "var(--accent-cyan)"; // 100 ~ 200ms: 良好 (青蓝)
  if (ms < 350) return "var(--accent-orange)"; // 200 ~ 350ms: 中等 (琥珀橙)
  if (ms < 600) return "#f97316"; // 350 ~ 600ms: 偏慢 (深橙)
  return "var(--accent-red)"; // >= 600ms: 高延迟 (红色)
}


const isTesting = computed(() => speedtestStore.testingNodes.has(props.nodeTag));
const isLatencyTesting = computed(() => speedtestStore.testingLatencyNodes.has(props.nodeTag));
const latencyColor = computed(() => getLatencyColor(props.latency));
</script>

<style scoped>
.node-card {
  position: relative;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--duration-fast) cubic-bezier(0.16, 1, 0.3, 1);
  user-select: none;
}

.node-card:hover {
  background: var(--layer-2);
  border-color: var(--border-normal);
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.node-card.non-selectable {
  cursor: default;
}

.node-card.non-selectable:hover {
  transform: none;
}

.node-card.active {
  border-color: var(--accent-blue);
  background: var(--accent-blue-glow);
  box-shadow: 0 0 12px rgba(79, 140, 255, 0.2);
}

.node-card.non-selectable.active {
  border-color: var(--accent-cyan);
  background: var(--accent-cyan-glow);
  box-shadow: 0 0 12px var(--accent-cyan-glow);
}

/* ==================== Grid 网格布局形态 ==================== */
.node-card.grid {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 10px 12px;
  min-height: 76px;
  gap: 8px;
}

.node-card.grid .node-main-info {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: 8px;
}

.node-card.grid .node-badge-title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.node-card.grid .node-title {
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-card.grid .node-metrics-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid rgba(255, 255, 255, 0.04);
  padding-top: 6px;
}

/* ==================== List 紧凑行布局形态 ==================== */
.node-card.list {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 14px;
  gap: 16px;
}

.node-card.list:hover {
  transform: translateX(2px);
}

.node-card.list .node-main-info {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1;
}

.node-card.list .node-badge-title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.node-card.list .node-title {
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-card.list .node-metrics-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-shrink: 0;
}

/* ==================== 通用元素样式 ==================== */
.protocol-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
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

.node-card.active .protocol-badge {
  background: var(--accent-blue);
  color: var(--text-on-accent);
  border-color: transparent;
}

.node-card.non-selectable.active .protocol-badge {
  background: var(--accent-cyan);
  color: #0d1117;
  border-color: transparent;
}

.select-badge-wrapper {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.active-check-circle {
  width: 18px;
  height: 18px;
  background: var(--accent-blue);
  color: #fff;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 6px var(--accent-blue);
}

.auto-active-badge {
  font-size: 9px;
  padding: 1px 5px;
  background: var(--accent-cyan-glow);
  color: var(--accent-cyan);
  border: 1px solid var(--accent-cyan);
  border-radius: var(--radius-xs);
  font-weight: var(--weight-bold);
}

.metrics-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.latency-box {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: var(--weight-medium);
  font-family: var(--font-mono);
}

.latency-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.speed-box {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 10px;
  color: var(--accent-cyan);
  font-family: var(--font-mono);
  background: color-mix(in srgb, var(--accent-cyan-vivid) 8%, transparent);
  padding: 1px 4px;
  border-radius: var(--radius-xs);
}

.card-actions {
  display: flex;
  align-items: center;
  gap: 4px;
}

.card-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-xs);
  border: 1px solid var(--border-subtle);
  background: var(--layer-2);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.card-action-btn:hover:not(:disabled) {
  background: var(--layer-3);
  color: var(--text-primary);
  border-color: var(--border-normal);
}

.card-action-btn.ping-btn:hover:not(:disabled) {
  color: var(--accent-orange);
  border-color: var(--accent-orange);
}

/* 置顶收藏星标：常态弱化，激活金色高亮 */
.card-action-btn.pin-btn {
  color: var(--text-tertiary);
}

.card-action-btn.pin-btn:hover:not(:disabled) {
  color: #f5c518;
  border-color: #f5c518;
}

.card-action-btn.pin-btn.pinned {
  color: #f5c518;
  border-color: rgba(245, 197, 24, 0.45);
  background: rgba(245, 197, 24, 0.08);
}

.card-action-btn.speed-btn:hover:not(:disabled) {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan);
}

.card-action-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.spin-icon {
  display: inline-block;
  animation: spin 1s linear infinite;
  font-size: 10px;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>

