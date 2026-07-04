<template>
  <div class="semantic-rule-card glass-effect" :class="record.type">
    <div class="card-icon">{{ record.icon }}</div>
    <div class="card-info">
      <div class="card-domain" :title="record.domain">{{ record.domain }}</div>
      <div class="card-semantic">{{ record.semanticText }}</div>
    </div>
    <div class="card-meta">
      <span class="outbound-badge">{{ record.outbound }}</span>
      <span class="time-stamp">{{ record.timestamp }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { SemanticAuditRecord } from "@/utils/semantic-translator";

defineProps<{
  record: SemanticAuditRecord;
}>();
</script>

<style scoped>
.semantic-rule-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  transition: all 0.2s ease;
}

.semantic-rule-card:hover {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.15);
}

.semantic-rule-card.proxied {
  border-left: 4px solid #00f2fe;
}
.semantic-rule-card.direct {
  border-left: 4px solid #4ade80;
}
.semantic-rule-card.blocked {
  border-left: 4px solid #f87171;
}

.card-icon {
  font-size: 20px;
}

.card-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.card-domain {
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
}

.card-semantic {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

.card-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
}

.outbound-badge {
  font-size: 11px;
  padding: 2px 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 4px;
  color: #00f2fe;
  font-weight: 600;
}

.time-stamp {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.35);
}
</style>
