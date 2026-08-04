<script setup lang="ts">
/**
 * 安全审计视图（语义化安全看板 + 极客内核日志流）
 * 作者: TanXiang
 */
import { ref, onActivated, onDeactivated } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { subscribeConnections } from "@/api/clash-ws";
import {
  translateConnection,
  type SemanticAuditRecord,
} from "@/utils/semantic-translator";
import SemanticRuleCard from "@/components/audit/SemanticRuleCard.vue";
import RawLogStream from "@/components/audit/RawLogStream.vue";

const connectionStore = useConnectionStore();
const viewMode = ref<"semantic" | "raw">("semantic");
const isPaused = ref(false);

const auditRecords = ref<SemanticAuditRecord[]>([]);

let unsub: (() => void) | null = null;

onActivated(() => {
  if (unsub) return;
  unsub = subscribeConnections((payload) => {
    if (isPaused.value) return;

    if (payload.connections && payload.connections.length > 0) {
      for (const conn of payload.connections.slice(0, 5)) {
        const domain = conn.destination || "未知主机";
        const outbound = conn.outbound || "direct";
        const ruleMatched = conn.rule || "default";

        const item = translateConnection(domain, outbound, ruleMatched);
        const record: SemanticAuditRecord = {
          ...item,
          id: Math.random().toString(36).substring(2, 9),
          timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
        };

        auditRecords.value.unshift(record);
        if (record.type === "proxied") connectionStore.stats.today_proxied++;
        else if (record.type === "direct") connectionStore.stats.today_direct++;
        else if (record.type === "blocked") connectionStore.stats.today_blocked++;
      }

      if (auditRecords.value.length > 200) {
        auditRecords.value = auditRecords.value.slice(0, 200);
      }
    }
  });
});

onDeactivated(() => {
  if (unsub) {
    unsub();
    unsub = null;
  }
});
</script>

<template>
  <div class="audit-view">
    <header class="page-header">
      <div class="title-area">
        <h1>🔍 安全审计看板</h1>
        <p class="subtitle">将冰冷数据归一翻译为自然语言，全景掌控网络与 DNS 解析规则</p>
      </div>
      <div class="tab-group">
        <button
          class="tab-btn"
          :class="{ active: viewMode === 'semantic' }"
          @click="viewMode = 'semantic'"
        >
          🛡️ 语义化看板
        </button>
        <button
          class="tab-btn"
          :class="{ active: viewMode === 'raw' }"
          @click="viewMode = 'raw'"
        >
          🧑‍💻 极客原始日志
        </button>
      </div>
    </header>

    <!-- 顶部连接数统计 -->
    <div class="stats-banner">
      <div class="banner-card proxied">
        <span class="val">{{ connectionStore.stats.today_proxied }}</span>
        <span class="label">🚀 节点加密代理</span>
      </div>
      <div class="banner-card direct">
        <span class="val">{{ connectionStore.stats.today_direct }}</span>
        <span class="label">🎯 大陆穿透直连</span>
      </div>
      <div class="banner-card blocked">
        <span class="val">{{ connectionStore.stats.today_blocked }}</span>
        <span class="label">🚫 恶意域名阻断</span>
      </div>
    </div>

    <!-- 主面板区 -->
    <main class="audit-main">
      <div
        v-if="viewMode === 'semantic'"
        class="semantic-feed-container"
        @mouseenter="isPaused = true"
        @mouseleave="isPaused = false"
      >
        <div class="feed-header">
          <span class="feed-title">实时网络连接规则流</span>
          <span class="pause-hint" v-if="isPaused">⏸️ 鼠标悬停已自动暂停流</span>
        </div>

        <div v-if="auditRecords.length === 0" class="empty-feed">
          尚无网络连接记录，请先使用网络浏览网页或发起请求...
        </div>

        <div v-else class="feed-list">
          <SemanticRuleCard
            v-for="rec in auditRecords"
            :key="rec.id"
            :record="rec"
          />
        </div>
      </div>

      <RawLogStream v-else />
    </main>
  </div>
</template>

<style scoped>
.audit-view {
  padding: var(--space-5);
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

/* 统计横幅 */
.stats-banner {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--space-4);
}

.banner-card {
  padding: var(--space-4) var(--space-5);
  border-radius: var(--radius-lg);
  background: var(--surface-raised);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.banner-card .val {
  font-size: var(--text-2xl);
  font-weight: var(--weight-bold);
}

.banner-card.proxied .val { color: var(--accent-cyan-vivid); }
.banner-card.direct .val { color: var(--status-success); }
.banner-card.blocked .val { color: var(--status-danger); }

.banner-card .label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

/* 主面板 */
.audit-main {
  flex: 1;
  overflow: hidden;
}

.semantic-feed-container {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  height: 100%;
}

.feed-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.feed-title {
  font-weight: var(--weight-semibold);
}

.pause-hint {
  font-size: var(--text-xs);
  color: var(--status-warning);
}

.empty-feed {
  padding: var(--space-10);
  text-align: center;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.feed-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  overflow-y: auto;
  flex: 1;
}
</style>
