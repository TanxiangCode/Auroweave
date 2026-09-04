<script setup lang="ts">
/**
 * 流量统计大盘 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定、数据初始化
 */
import { onMounted } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { storeToRefs } from "pinia";
import SvgIcon from "@/components/common/SvgIcon.vue";
import { useConfirm } from "@/composables/useConfirm";

import StatsOverviewCards from "./components/StatsOverviewCards.vue";
import TrafficBarChart from "./components/TrafficBarChart.vue";
import ProtocolDonutChart from "./components/ProtocolDonutChart.vue";
import AppTrafficList from "./components/AppTrafficList.vue";

import { useTrafficHistory } from "./hooks/useTrafficHistory";
import { useAppTraffic } from "./hooks/useAppTraffic";

import { useToast } from "@/composables/useToast";
import { invoke } from "@tauri-apps/api/core";

const connectionStore = useConnectionStore();
const { totalDownload, totalUpload } = storeToRefs(connectionStore);
const toast = useToast();

// === Hook 初始化 ===

const { timeDimension, chartData, fetchTrafficHistory } = useTrafficHistory();
const { topApps, fetchAppTraffic } = useAppTraffic();

// === 事件处理 ===

/** 清空大盘数据（先确保后端数据库清除成功，再清本地，避免前后端不一致） */
async function clearStats() {
  const confirmed = await useConfirm().ask({
    title: "清空流量统计",
    message: "确定要清空累计的历史流量统计与数据库记录吗？该操作不可恢复。",
    confirmText: "清空",
    level: "danger",
  });
  if (!confirmed) {
    return;
  }
  try {
    await invoke("stats_clear_all");
    // 后端清除成功后才清理本地累计数据
    totalDownload.value = 0;
    totalUpload.value = 0;
    localStorage.setItem("auroweave_total_download", "0");
    localStorage.setItem("auroweave_total_upload", "0");
    await fetchTrafficHistory();
    await fetchAppTraffic();
    toast.success("流量数据已重置", "本地历史记录已全部清空");
  } catch (e: any) {
    toast.error("重置失败", e?.message || String(e));
  }
}

// === 生命周期 ===

onMounted(() => {
  fetchTrafficHistory();
  fetchAppTraffic();
});
</script>

<template>
  <div class="stats-container">
    <!-- 头部（page-header 标准节奏） -->
    <header class="page-header">
      <div class="title-area">
        <h1><SvgIcon name="stats" :size="24" class="title-icon" /> 流量统计大盘</h1>
        <p class="subtitle">按小时聚合的流量趋势与应用排行，数据本地持久化</p>
      </div>
      <div class="header-actions">
        <button class="btn-clear" @click="clearStats" title="清空所有累计历史流量统计">
          <SvgIcon name="refresh" :size="12" class="icon-gap" />
        重置数据
      </button>
    </header>

    <!-- 顶部核心累计看板 -->
    <StatsOverviewCards
      :total-download="totalDownload"
      :total-upload="totalUpload"
    />

    <!-- 图表面板网格 -->
    <section class="charts-grid">
      <!-- 分时流量柱状图 -->
      <TrafficBarChart
        :chart-data="chartData"
        :time-dimension="timeDimension"
        @update:time-dimension="timeDimension = $event"
      />

      <!-- 协议配额环形图 -->
      <ProtocolDonutChart
        :total-download="totalDownload"
        :total-upload="totalUpload"
      />

      <!-- 应用程序流量 Top 10 -->
      <AppTrafficList :top-apps="topApps" />
    </section>
  </div>
</template>

<style scoped>
.stats-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 20px;
  padding: 24px;
  overflow-y: auto;
}

.stats-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.stats-header h2 {
  font-size: var(--text-lg);
  font-weight: var(--weight-bold);
}

.sub-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  margin-top: 4px;
  display: block;
}

.btn-clear {
  display: flex;
  align-items: center;
  padding: 6px 14px;
  border-radius: var(--radius-sm);
  background: transparent;
  border: 1px solid var(--border-normal);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-clear:hover {
  background: var(--accent-red-glow);
  border-color: var(--accent-red);
  color: var(--accent-red);
}

.icon-gap {
  margin-right: 4px;
}

.charts-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}

/* 柱状图占满整行 */
.charts-grid > :first-child {
  grid-column: 1 / -1;
}
</style>
