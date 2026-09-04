<script setup lang="ts">
/**
 * 安全审核与连接审计看板 — 主入口
 * 作者: TanXiang
 *
 * 职责：全景仪表盘、多维过滤流控、三重视图协同与溯源抽屉交互
 */
import { ref, computed } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useConnectionStore } from "@/stores/connection.store";
import { useConnectionAudit } from "./hooks/useConnectionAudit";
import AuditOverviewBanner from "@/components/audit/AuditOverviewBanner.vue";
import AuditFilterToolbar from "@/components/audit/AuditFilterToolbar.vue";
import SemanticRuleCard from "@/components/audit/SemanticRuleCard.vue";
import ConnectionTable from "@/components/audit/ConnectionTable.vue";
import ConnectionDetailDrawer from "@/components/audit/ConnectionDetailDrawer.vue";
import RawLogStream from "@/components/audit/RawLogStream.vue";
import ConnectivityPanel from "@/components/audit/ConnectivityPanel.vue";
import type { SemanticAuditRecord } from "@/utils/semantic-translator";

const connectionStore = useConnectionStore();
const {
  isPaused,
  activeRecords,
  allRecords,
  handleCloseConnection,
  handleCloseAllConnections,
  clearHistory,
} = useConnectionAudit();

// 视图与过滤状态
const viewMode = ref<"semantic" | "table" | "raw" | "connectivity">("semantic");
const searchQuery = ref("");
const statusFilter = ref<"all" | "proxied" | "direct" | "blocked">("all");
const protocolFilter = ref<"all" | "tcp" | "udp">("all");
// 悬停自动暂停开关（默认关闭，由用户自主开启）
const autoPauseOnHover = ref(false);
// 手动暂停标志：用户点击暂停按钮后，鼠标离开不再自动恢复
const manuallyPaused = ref(false);

function handleTogglePause() {
  // 点击"暂停/恢复"按钮时切换手动暂停标志
  manuallyPaused.value = !isPaused.value;
  isPaused.value = !isPaused.value;
}

// 选中的单条连接（详情抽屉）
const selectedRecord = ref<SemanticAuditRecord | null>(null);

// 活跃连接 ID 集合
const activeIdSet = computed(() => new Set(activeRecords.value.map((r) => r.id)));

// 过滤后的记录列表
// 性能：/connections WS 每秒推全量快照触发本 computed 重算——无任何过滤
// 条件时短路返回原引用，跳过整表 filter 链与数组重建
const filteredRecords = computed(() => {
  const noStatus = statusFilter.value === "all";
  const noProtocol = protocolFilter.value === "all";
  const noSearch = !searchQuery.value.trim();
  if (noStatus && noProtocol && noSearch) {
    return allRecords.value;
  }

  let list = allRecords.value;

  // 1. 状态胶囊过滤
  if (!noStatus) {
    list = list.filter((r) => r.type === statusFilter.value);
  }

  // 2. 协议过滤
  if (!noProtocol) {
    list = list.filter((r) => (r.network || "tcp").toLowerCase() === protocolFilter.value);
  }

  // 3. 关键词多维搜索 (进程名 / 域名 / IP / 规则 / 节点)
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((r) => {
      return (
        r.domain.toLowerCase().includes(q) ||
        r.appDisplayName.toLowerCase().includes(q) ||
        r.process.toLowerCase().includes(q) ||
        r.outbound.toLowerCase().includes(q) ||
        r.ruleMatched.toLowerCase().includes(q) ||
        (r.destinationIP && r.destinationIP.includes(q))
      );
    });
  }

  return list;
});

function handleSelectRecord(record: SemanticAuditRecord) {
  selectedRecord.value = record;
}

function handleMouseEnter() {
  if (autoPauseOnHover.value && !manuallyPaused.value) {
    isPaused.value = true;
  }
}

function handleMouseLeave() {
  // 手动暂停时不被悬停逻辑自动恢复
  if (autoPauseOnHover.value && !manuallyPaused.value) {
    isPaused.value = false;
  }
}
</script>

<template>
  <div class="audit-view">
    <!-- 页面头部 -->
    <header class="page-header">
      <div class="title-area">
        <h1><BaseIcon name="ShieldCheck" :size="24" class="title-icon" /> 安全审计</h1>
        <p class="subtitle">实时连接语义流、流量溯源与连通性检测；悬停暂停以细读链路</p>
      </div>
    </header>

    <!-- 顶部紧凑全景指标看板与流控 -->
    <AuditOverviewBanner
      :active-count="activeRecords.length"
      :proxied-count="connectionStore.stats.today_proxied"
      :direct-count="connectionStore.stats.today_direct"
      :blocked-count="connectionStore.stats.today_blocked"
      :download-speed="connectionStore.formatSpeed(connectionStore.rawDownloadSpeed)"
      :upload-speed="connectionStore.formatSpeed(connectionStore.rawUploadSpeed)"
      :is-paused="isPaused"
      @toggle-pause="handleTogglePause"
      @close-all="handleCloseAllConnections"
      @clear-history="clearHistory"
    />

    <!-- 多维过滤工具栏 (第二行整合：事件统计 + 悬停暂停开关 + 视图Tab)
         组件内部在 raw 视图下自动隐藏搜索/过滤行（原始日志流不走过滤逻辑） -->
    <AuditFilterToolbar
      v-model:search-query="searchQuery"
      v-model:status-filter="statusFilter"
      v-model:protocol-filter="protocolFilter"
      v-model:view-mode="viewMode"
      v-model:auto-pause-on-hover="autoPauseOnHover"
      :total-count="allRecords.length"
      :match-count="filteredRecords.length"
      :is-paused="isPaused"
    />

    <!-- 主展示区 (flex: 1 撑满剩余高度) -->
    <main class="audit-main">
      <!-- 视图 1：智能语义流 -->
      <div
        v-if="viewMode === 'semantic'"
        class="semantic-feed-container"
        @mouseenter="handleMouseEnter"
        @mouseleave="handleMouseLeave"
      >
        <div v-if="filteredRecords.length === 0" class="empty-feed glass-effect">
          {{ allRecords.length === 0 ? '尚无网络会话记录，发起请求或浏览网页时将实时显示...' : '未找到符合过滤条件的网络连接' }}
        </div>

        <div v-else class="feed-list">
          <SemanticRuleCard
            v-for="rec in filteredRecords"
            :key="rec.id"
            :record="rec"
            @select="handleSelectRecord"
          />
        </div>
      </div>

      <!-- 视图 2：连接拓扑明细表 -->
      <ConnectionTable
        v-else-if="viewMode === 'table'"
        :records="filteredRecords"
        :active-id-set="activeIdSet"
        @select="handleSelectRecord"
        @close="handleCloseConnection"
      />

      <!-- 视图 3：底层内核原始日志 -->
      <RawLogStream v-else-if="viewMode === 'raw'" />

      <!-- 视图 4：连通性与出口检测 -->
      <ConnectivityPanel v-else />
    </main>

    <!-- 连接详情与溯源抽屉 -->
    <ConnectionDetailDrawer
      :record="selectedRecord"
      :is-active="selectedRecord ? activeIdSet.has(selectedRecord.id) : false"
      @close="selectedRecord = null"
      @disconnect="handleCloseConnection"
    />
  </div>
</template>

<style scoped>
.audit-view {
  padding: var(--space-4) var(--space-5);
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 10px;
  box-sizing: border-box;
}

.audit-main {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.semantic-feed-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.feed-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  padding-right: 2px;
}

.empty-feed {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: rgba(255, 255, 255, 0.35);
  font-size: 13px;
  border-radius: 12px;
  border: 1px dashed rgba(255, 255, 255, 0.08);
}
</style>
