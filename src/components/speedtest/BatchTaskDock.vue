<template>
  <div v-if="hasActiveTasks" class="batch-task-dock">
    <div class="task-dock-grid">
      <!-- 1. 批量测延迟任务进度 -->
      <BatchProgressCard
        v-if="speedtestStore.latencyBatchProgress"
        :visible="true"
        :progress="speedtestStore.latencyBatchProgress"
        action-label="延迟测试"
        cancel-label="取消"
        @cancel="speedtestStore.cancelLatencyBatch"
      />

      <!-- 2. 批量吞吐量测速任务进度 -->
      <BatchProgressCard
        v-if="speedtestStore.isBatchTesting && speedtestStore.batchProgress"
        :visible="true"
        :progress="speedtestStore.batchProgress"
        action-label="正在测速"
        cancel-label="取消"
        @cancel="speedtestStore.cancelBatch"
      />

      <!-- 3. 流媒体解锁检测任务进度 -->
      <BatchProgressCard
        v-if="unlockStore.isBatchChecking && unlockStore.batchProgress"
        :visible="true"
        :progress="unlockStore.batchProgress"
        action-label="解锁检测"
        cancel-label="取消"
        @cancel="unlockStore.cancelBatch"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 批量后台任务聚合吸底 Dock (BatchTaskDock)
 * 作者: TanXiang
 *
 * 聚合管理延迟测试、吞吐量测速、流媒体解锁检测 3 个批量异步任务：
 * - 当只有 1 个任务运行时，全宽平铺，界面舒展大方；
 * - 当有 2~3 个任务并发执行时，采用横向自适应网格 (CSS Grid) 并排平分展示；
 * - 总体高度严格限制在单行紧凑高度（约 42px~46px），彻底根治 3 个卡片纵向堆叠占满视口的痛点；
 * - 无任务时零 DOM 占用，不影响上方节点列表区域。
 */
import { computed } from "vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useUnlockStore } from "@/stores/unlock.store";
import BatchProgressCard from "./BatchProgressCard.vue";

const speedtestStore = useSpeedtestStore();
const unlockStore = useUnlockStore();

const hasActiveTasks = computed(() => {
  const hasLatency = !!speedtestStore.latencyBatchProgress;
  const hasSpeedtest = speedtestStore.isBatchTesting && !!speedtestStore.batchProgress;
  const hasUnlock = unlockStore.isBatchChecking && !!unlockStore.batchProgress;
  return hasLatency || hasSpeedtest || hasUnlock;
});
</script>

<style scoped>
.batch-task-dock {
  position: sticky;
  bottom: 0;
  left: 0;
  right: 0;
  z-index: 20;
  margin-top: var(--space-2);
  padding-top: var(--space-2);
  background: var(--bg-surface-glass, rgba(18, 18, 20, 0.85));
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-top: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
}

.task-dock-grid {
  display: grid;
  /* 宽屏并排平分展示，空间不足自适应折行，单任务占满 */
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: var(--space-2);
  align-items: center;
}
</style>
