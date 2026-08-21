<template>
  <div
    class="semantic-rule-card glass-effect"
    :class="[record.type, record.riskLevel]"
    @click="$emit('select', record)"
  >
    <!-- 左侧：应用图标与主体信息 -->
    <div class="card-left">
      <div class="app-icon-wrap" :title="record.process || record.appDisplayName">
        <BaseIcon :name="record.appIcon || 'Cpu'" :size="18" class="semantic-app-icon" />
      </div>


      <div class="main-info">
        <div class="title-row">
          <span class="app-name">{{ record.appDisplayName }}</span>
          <span class="domain-name" :title="record.domain">{{ record.domain }}</span>
          <span class="port-tag" v-if="record.port">:{{ record.port }}</span>
        </div>

        <div class="semantic-desc">
          {{ record.semanticDesc }}
        </div>

        <!-- 标签徽章流 -->
        <div class="badges-row">
          <span
            v-for="(b, idx) in record.securityBadges"
            :key="idx"
            class="security-badge"
            :class="getBadgeClass(b)"
          >
            {{ b }}
          </span>
          <span class="rule-badge" v-if="record.ruleMatched">
            {{ record.ruleMatched }}
          </span>
        </div>
      </div>
    </div>

    <!-- 右侧：出站链路与流量速率元数据 -->
    <div class="card-right">
      <div class="outbound-pill" :class="record.type">
        <span class="outbound-icon">{{ record.type === 'direct' ? '' : record.type === 'blocked' ? '' : '' }}</span>
        <span class="outbound-name">{{ record.outbound }}</span>
      </div>

      <div class="traffic-metrics" v-if="record.download_bytes > 0 || record.upload_bytes > 0">
        <span class="traffic-text down">↓ {{ formatBytes(record.download_bytes) }}</span>
        <span class="traffic-text up">↑ {{ formatBytes(record.upload_bytes) }}</span>
      </div>

      <span class="time-stamp">{{ record.timestamp }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import type { SemanticAuditRecord } from "@/utils/semantic-translator";


defineProps<{
  record: SemanticAuditRecord;
}>();

defineEmits<{
  (e: "select", record: SemanticAuditRecord): void;
}>();

function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getBadgeClass(badge: string): string {
  if (badge.includes("TLS") || badge.includes("加密") || badge.includes("防护")) return "safe";
  if (badge.includes("直连") || badge.includes("穿透")) return "direct";
  if (badge.includes("广告") || badge.includes("拦截") || badge.includes("明文")) return "danger";
  return "default";
}
</script>

<style scoped>
.semantic-rule-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 14px;
  padding: 10px 14px;
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
  overflow: hidden;
  /* 核心修复：锁定卡片最小高度与防压缩，防止多条目时被压扁 */
  flex-shrink: 0;
  min-height: 64px;
  box-sizing: border-box;
}

.semantic-rule-card:hover {
  background: rgba(255, 255, 255, 0.055);
  border-color: rgba(255, 255, 255, 0.15);
  transform: translateY(-1px);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
}

.semantic-rule-card.proxied {
  border-left: 3px solid #00f2fe;
}
.semantic-rule-card.direct {
  border-left: 3px solid #10b981;
}
.semantic-rule-card.blocked {
  border-left: 3px solid #f87171;
}

.card-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.app-icon-wrap {
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  flex-shrink: 0;
}

.main-info {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
  flex: 1;
}

.title-row {
  display: flex;
  align-items: baseline;
  gap: 6px;
  flex-wrap: wrap;
}

.app-name {
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.95);
}

.domain-name {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.65);
  font-family: monospace;
  max-width: 380px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.port-tag {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.35);
  font-family: monospace;
}

.semantic-desc {
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.55);
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 520px;
}

.badges-row {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
}

.security-badge, .rule-badge {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.07);
  color: rgba(255, 255, 255, 0.6);
}

.security-badge.safe {
  color: #00f2fe;
  background: rgba(0, 242, 254, 0.08);
  border-color: rgba(0, 242, 254, 0.2);
}

.security-badge.direct {
  color: #10b981;
  background: rgba(16, 185, 129, 0.08);
  border-color: rgba(16, 185, 129, 0.2);
}

.security-badge.danger {
  color: #f87171;
  background: rgba(248, 113, 113, 0.08);
  border-color: rgba(248, 113, 113, 0.2);
}

.card-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  flex-shrink: 0;
}

.outbound-pill {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 7px;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 600;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.outbound-pill.proxied {
  color: #00f2fe;
  border-color: rgba(0, 242, 254, 0.25);
  background: rgba(0, 242, 254, 0.08);
}

.outbound-pill.direct {
  color: #10b981;
  border-color: rgba(16, 185, 129, 0.25);
  background: rgba(16, 185, 129, 0.08);
}

.outbound-pill.blocked {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.25);
  background: rgba(248, 113, 113, 0.08);
}

.traffic-metrics {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.traffic-text.down { color: #00f2fe; }
.traffic-text.up { color: #a78bfa; }

.time-stamp {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.3);
}
</style>
