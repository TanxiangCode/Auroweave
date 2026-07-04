<script setup lang="ts">
/**
 * 安全审计视图（语义化安全看板 + 极客内核日志流）
 * 作者: TanXiang
 */
import { ref, onMounted, onUnmounted } from "vue";
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

onMounted(() => {
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

onUnmounted(() => {
  if (unsub) unsub();
});
</script>

<template>
  <div class="audit-view">
    <header class="audit-header">
      <div class="title-area">
        <h1>🔍 安全审计看板</h1>
        <p class="subtitle">将冰冷数据归一翻译为自然语言，全景掌控网络与 DNS 解析规则</p>
      </div>

      <div class="header-actions">
        <button
          class="btn-toggle"
          :class="{ active: viewMode === 'semantic' }"
          @click="viewMode = 'semantic'"
        >
          🛡️ 语义化看板
        </button>
        <button
          class="btn-toggle"
          :class="{ active: viewMode === 'raw' }"
          @click="viewMode = 'raw'"
        >
          🧑‍💻 极客原始日志
        </button>
      </div>
    </header>

    <!-- 顶部连接数真实统计框 -->
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
      <!-- 语义化看板模式 -->
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

      <!-- 极客原始日志流模式 -->
      <RawLogStream v-else />
    </main>
  </div>
</template>

<style scoped>
.audit-view {
  padding: 48px 24px 24px 24px;
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.audit-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title-area h1 {
  font-size: 20px;
  font-weight: 700;
}

.subtitle {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  margin-top: 4px;
}

.header-actions {
  display: flex;
  gap: 8px;
  background: rgba(255, 255, 255, 0.04);
  padding: 4px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.btn-toggle {
  padding: 6px 14px;
  border-radius: 8px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-toggle.active {
  background: rgba(0, 242, 254, 0.15);
  color: #fff;
  font-weight: 600;
}

.stats-banner {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.banner-card {
  padding: 16px 20px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.03);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.banner-card .val {
  font-size: 24px;
  font-weight: 700;
}

.banner-card.proxied .val { color: #00f2fe; }
.banner-card.direct .val { color: #4ade80; }
.banner-card.blocked .val { color: #f87171; }

.banner-card .label {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

.audit-main {
  flex: 1;
  overflow: hidden;
}

.semantic-feed-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
}

.feed-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.7);
}

.feed-title {
  font-weight: 600;
}

.pause-hint {
  font-size: 11px;
  color: #fbbf24;
}

.empty-feed {
  padding: 40px;
  text-align: center;
  color: rgba(255, 255, 255, 0.3);
  font-size: 13px;
}

.feed-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
  flex: 1;
}
</style>
