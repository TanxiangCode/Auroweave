<template>
  <div class="connection-table-container glass-effect">
    <div v-if="records.length === 0" class="empty-state">
      暂无匹配的网络连接记录
    </div>

    <div v-else class="table-wrap">
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
          <tr
            v-for="rec in records"
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

            <!-- 累计传输 -->
            <td class="td-traffic">
              <div class="traffic-cell">
                <span class="traffic-down">↓ {{ formatBytes(rec.download_bytes) }}</span>
                <span class="traffic-up">↑ {{ formatBytes(rec.upload_bytes) }}</span>
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
                  
                </button>
                <button
                  v-if="isActive(rec.id)"
                  class="btn-icon-action danger"
                  title="切断此连接"
                  @click="$emit('close', rec.id)"
                >
                  
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
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
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.06);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.table-wrap {
  flex: 1;
  overflow-y: auto;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  text-align: left;
}

thead {
  position: sticky;
  top: 0;
  background: rgba(18, 20, 28, 0.95);
  backdrop-filter: blur(10px);
  z-index: 2;
}

th {
  padding: 10px 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.5);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  white-space: nowrap;
}

td {
  padding: 10px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  color: rgba(255, 255, 255, 0.85);
  vertical-align: middle;
}

.table-row {
  cursor: pointer;
  transition: background 0.15s ease;
}

.table-row:hover {
  background: rgba(255, 255, 255, 0.04);
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
  background: rgba(255, 255, 255, 0.2);
}

.app-cell {
  display: flex;
  align-items: center;
  gap: 8px;
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
  color: #fff;
}

.proc-name {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
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
  color: rgba(255, 255, 255, 0.9);
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
  color: rgba(255, 255, 255, 0.35);
  font-family: monospace;
}

.proto-tag {
  padding: 1px 4px;
  background: rgba(255, 255, 255, 0.06);
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
.outbound-badge.blocked { color: #f87171; background: rgba(248, 113, 113, 0.1); }

.node-tag {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.7);
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rule-name {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
  padding: 2px 6px;
  background: rgba(255, 255, 255, 0.04);
  border-radius: 4px;
}

.traffic-cell {
  display: flex;
  flex-direction: column;
  gap: 1px;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

.traffic-down { color: var(--accent-cyan-vivid); }
.traffic-up { color: #a78bfa; }

.duration-text {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
}

.action-btns {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-icon-action {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 4px 6px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-icon-action:hover {
  background: rgba(255, 255, 255, 0.15);
  border-color: rgba(255, 255, 255, 0.3);
}

.btn-icon-action.danger:hover {
  background: rgba(239, 68, 68, 0.2);
  border-color: #ef4444;
}

.empty-state {
  padding: 60px;
  text-align: center;
  color: rgba(255, 255, 255, 0.35);
  font-size: 13px;
}
</style>
