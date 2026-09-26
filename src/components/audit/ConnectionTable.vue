<template>
  <div class="connection-table-container glass-effect">
    <EmptyState v-if="records.length === 0" icon="Globe" title="暂无匹配的网络连接记录" description="发起请求或浏览网页时将实时显示会话" />

    <div v-else ref="tableWrap" class="table-wrap" @scroll.passive="onScroll">
      <table class="data-table">
        <thead>
          <tr>
            <th class="th-status">状态</th>
            <th class="th-app">应用进程</th>
            <th class="th-dest">目标主机 / IP</th>
            <th class="th-outbound">出站节点 / 转发链</th>
            <th class="th-rule">匹配规则</th>
            <th class="th-traffic">累计传输</th>
            <th class="th-duration">持续时长</th>
            <th class="th-actions">操作</th>
          </tr>
        </thead>
        <tbody>
          <!-- 虚拟滚动：窗口外上占位（撑起滚动高度） -->
          <tr v-if="topPadPx > 0" :style="{ height: topPadPx + 'px' }" aria-hidden="true">
            <td colspan="8" class="pad-cell"></td>
          </tr>
          <tr
            v-for="rec in visibleRecords"
            :key="rec.id"
            class="table-row"
            :class="{ active: isActive(rec.id) }"
            @click="$emit('select', rec)"
          >
            <!-- 状态灯 -->
            <td class="td-status">
              <span
                class="status-dot"
                :class="{ online: isActive(rec.id), closed: !isActive(rec.id) }"
                :title="isActive(rec.id) ? '活跃会话中' : '已断开/历史记录'"
              ></span>
            </td>

            <!-- 应用进程 -->
            <td class="td-app">
              <div class="app-cell">
                <BaseIcon :name="rec.appIcon || 'Cpu'" :size="16" class="table-app-icon" />
                <div class="app-info">

                  <span class="app-name">{{ rec.appDisplayName }}</span>
                  <span class="proc-name" v-if="rec.process && rec.process !== rec.appDisplayName" :title="rec.process">
                    {{ rec.process }}
                  </span>
                </div>
              </div>
            </td>

            <!-- 目标主机 / IP / 端口 -->
            <td class="td-dest">
              <div class="dest-cell">
                <div class="dest-host" :title="rec.domain">{{ rec.domain }}</div>
                <div class="dest-meta">
                  <span class="ip-tag" v-if="rec.destinationIP">{{ rec.destinationIP }}:{{ rec.port }}</span>
                  <span class="proto-tag">{{ (rec.network || 'TCP').toUpperCase() }}</span>
                </div>
              </div>
            </td>

            <!-- 出站链路 -->
            <td class="td-outbound">
              <div class="outbound-cell">
                <span class="outbound-badge" :class="rec.type">
                  {{ rec.type === 'direct' ? ' 直连' : rec.type === 'blocked' ? ' 阻断' : ' 代理' }}
                </span>
                <span class="node-tag" :title="rec.chains?.join(' → ') || rec.outbound">
                  {{ rec.outbound }}
                </span>
              </div>
            </td>

            <!-- 匹配规则 -->
            <td class="td-rule">
              <span class="rule-name" :title="rec.rulePayload ? `${rec.ruleMatched} (${rec.rulePayload})` : rec.ruleMatched">
                {{ rec.ruleMatched }}
              </span>
            </td>

            <!-- 累计传输：下行/上行各占一行，行内不折行 -->
            <td class="td-traffic">
              <div class="traffic-cell">
                <span class="traffic-down" title="下行累计">↓ {{ formatBytes(rec.download_bytes) }}</span>
                <span class="traffic-up" title="上行累计">↑ {{ formatBytes(rec.upload_bytes) }}</span>
              </div>
            </td>

            <!-- 持续时长 -->
            <td class="td-duration">
              <span class="duration-text">{{ getDuration(rec.start) }}</span>
            </td>

            <!-- 操作 -->
            <td class="td-actions" @click.stop>
              <div class="action-btns">
                <button
                  class="btn-icon-action"
                  title="查看详细溯源信息"
                  @click="$emit('select', rec)"
                >
                  <BaseIcon name="Eye" :size="12" /> 溯源
                </button>
                <button
                  v-if="isActive(rec.id)"
                  class="btn-icon-action danger"
                  title="切断此连接"
                  @click="$emit('close', rec.id)"
                >
                  <BaseIcon name="X" :size="12" /> 切断
                </button>
                <span v-else class="action-closed-tag">已断开</span>
              </div>
            </td>
          </tr>
          <!-- 虚拟滚动：窗口外下占位 -->
          <tr v-if="bottomPadPx > 0" :style="{ height: bottomPadPx + 'px' }" aria-hidden="true">
            <td colspan="8" class="pad-cell"></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { formatBytes } from "@/utils/format";
import type { SemanticAuditRecord } from "@/utils/semantic-translator";


const props = defineProps<{
  records: SemanticAuditRecord[];
  activeIdSet: Set<string>;
}>();

defineEmits<{
  (e: "select", record: SemanticAuditRecord): void;
  (e: "close", id: string): void;
}>();

// ---- 窗口化虚拟滚动 ----
// /connections WS 每秒推全量快照，历史+活跃最多 ~500 行原全量渲染 DOM。
// 表格虚拟化方案：滚动容器监听 scrollTop，只渲染可见窗口 + 上下缓冲，
// 窗口外以单个撑高占位行代替（上占位/下占位），DOM 数量与总行数解耦。
const ROW_HEIGHT = 58; // td padding 10px×2 + 双行内容（实测布局）
const BUFFER_ROWS = 8; // 上下缓冲行数（快速滚动不露白）

const tableWrap = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportHeight = ref(600);

function onScroll() {
  if (tableWrap.value) {
    scrollTop.value = tableWrap.value.scrollTop;
  }
}

let ro: ResizeObserver | null = null;
onMounted(() => {
  if (tableWrap.value) {
    viewportHeight.value = tableWrap.value.clientHeight;
    ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        viewportHeight.value = entry.contentRect.height;
      }
    });
    ro.observe(tableWrap.value);
  }
});
onUnmounted(() => ro?.disconnect());

const startIndex = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - BUFFER_ROWS)
);
const endIndex = computed(() => {
  const visible = Math.ceil(viewportHeight.value / ROW_HEIGHT) + BUFFER_ROWS * 2;
  return Math.min(props.records.length, startIndex.value + visible);
});
const visibleRecords = computed(() =>
  props.records.slice(startIndex.value, endIndex.value)
);
const topPadPx = computed(() => startIndex.value * ROW_HEIGHT);
const bottomPadPx = computed(() =>
  Math.max(0, (props.records.length - endIndex.value) * ROW_HEIGHT)
);

// 记录数变化时收敛滚动位置（过滤后列表变短防越界空白）
watch(() => props.records.length, () => {
  if (tableWrap.value) {
    const maxScroll = Math.max(0, props.records.length * ROW_HEIGHT - viewportHeight.value);
    if (tableWrap.value.scrollTop > maxScroll) {
      tableWrap.value.scrollTop = maxScroll;
      scrollTop.value = maxScroll;
    }
  }
});

function isActive(id: string): boolean {
  return props.activeIdSet.has(id);
}

function getDuration(startTime: number): string {
  if (!startTime) return "0s";
  const sec = Math.max(0, Math.floor((Date.now() - startTime) / 1000));
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  const remainSec = sec % 60;
  return `${min}m ${remainSec}s`;
}
</script>

<style scoped>
.connection-table-container {
  height: 100%;
  border-radius: 14px;
  background: var(--surface-inset);
  border: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.table-wrap {
  flex: 1;
  /* fixed 列宽布局下表格可能宽于视口：纵向滚动 + 必要时横向滚动 */
  overflow-y: auto;
  overflow-x: auto;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  text-align: left;
  /* 列宽策略：窄窗口下浏览器默认按内容均摊，应用进程/出站/操作列被
     目标主机长域名挤窄。显式锁定次要列宽，弹性列声明 min-width 让
     表格按需溢出横向滚动而非压缩内容 */
  table-layout: fixed;
}

/* 状态灯列 */
.th-status, .td-status { width: 40px; }
/* 应用进程列：保证双行文本不换行 */
.th-app, .td-app { width: 170px; }
/* 目标主机列：弹性收缩 + 内部省略号 */
.th-dest, .td-dest { width: auto; min-width: 180px; }
/* 出站节点列 */
.th-outbound, .td-outbound { width: auto; min-width: 180px; }
/* 匹配规则列 */
.th-rule, .td-rule { width: auto; min-width: 130px; }
/* 累计传输列 */
.th-traffic, .td-traffic { width: 90px; }
/* 持续时长列 */
.th-duration, .td-duration { width: 60px; }
/* 操作列 */
.th-actions, .td-actions { width: 130px; }

.table-wrap {
  overflow-x: auto;
}

thead {
  position: sticky;
  top: 0;
  background: var(--surface-raised);
  backdrop-filter: blur(10px);
  z-index: 2;
}

th {
  padding: 10px 14px;
  font-weight: 600;
  color: var(--text-tertiary);
  border-bottom: 1px solid var(--border-normal);
  white-space: nowrap;
}

td {
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-primary);
  vertical-align: middle;
}

.table-row {
  cursor: pointer;
  transition: background 0.15s ease;
}

/* 虚拟滚动占位行：无边框无内容，仅撑滚动高度 */
.pad-cell {
  padding: 0 !important;
  border: none !important;
  line-height: 0;
}

.table-row:hover {
  background: var(--surface-raised);
}

.status-dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.status-dot.online {
  background: var(--accent-green);
  box-shadow: 0 0 6px rgba(16, 185, 129, 0.8);
}

.status-dot.closed {
  background: var(--text-tertiary);
}

.app-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.app-cell .table-app-icon {
  flex-shrink: 0;
}

.app-info {
  min-width: 0;
}

.app-icon {
  font-size: 16px;
}

.app-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.app-name {
  font-weight: 600;
  color: var(--text-primary);
}

.proc-name {
  font-size: 10px;
  color: var(--text-tertiary);
  font-family: monospace;
}

.dest-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.dest-host {
  font-family: monospace;
  font-weight: 500;
  color: var(--text-primary);
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dest-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
  color: var(--text-tertiary);
  font-family: monospace;
}

.proto-tag {
  padding: 1px 4px;
  background: var(--surface-raised);
  border-radius: 3px;
}

.outbound-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}

.outbound-badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  font-weight: 600;
}

.outbound-badge.proxied { color: var(--accent-cyan-vivid); background: color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent); }
.outbound-badge.direct { color: var(--accent-green); background: rgba(16, 185, 129, 0.1); }
.outbound-badge.blocked { color: var(--status-danger); background: color-mix(in srgb, var(--status-danger) 10%, transparent); }

.node-tag {
  font-size: 11px;
  color: var(--text-secondary);
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-name {
  font-size: 11px;
  color: var(--text-tertiary);
  padding: 2px 6px;
  background: var(--surface-raised);
  border-radius: 4px;
}

.traffic-cell {
  display: flex;
  flex-direction: column;
  gap: 1px;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  /* 下行/上行各占一行，行内不折行 */
  white-space: nowrap;
}

.traffic-down { color: var(--accent-cyan-vivid); }
.traffic-up { color: var(--accent-purple); }

.duration-text {
  font-size: 11px;
  color: var(--text-tertiary);
  white-space: nowrap;
}

.action-btns {
  display: flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}

.btn-icon-action {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: 6px;
  padding: 4px 7px;
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.btn-icon-action:hover {
  background: var(--surface-hover);
  border-color: var(--text-tertiary);
  color: var(--text-primary);
}

.btn-icon-action.danger {
  color: rgba(248, 113, 113, 0.9);
}

.btn-icon-action.danger:hover {
  background: color-mix(in srgb, var(--accent-red) 20%, transparent);
  border-color: var(--accent-red);
  color: var(--status-danger);
}

/* 已断开行：无切断按钮时占位，保持列视觉平衡 */
.action-closed-tag {
  font-size: 10px;
  color: var(--text-tertiary);
  padding: 4px 7px;
  white-space: nowrap;
}
</style>
