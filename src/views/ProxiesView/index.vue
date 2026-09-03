<script setup lang="ts">
/**
 * 代理节点视图 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定、Hook 协调
 * 业务逻辑全部委托至各子组件与 Hook
 */
import { onMounted, onActivated, onDeactivated, ref } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";
import SvgIcon from "@/components/common/SvgIcon.vue";

// 子组件
import GroupSidebar from "./components/GroupSidebar.vue";
import NodeToolbar from "./components/NodeToolbar.vue";
import NodeListPanel from "./components/NodeListPanel.vue";
import BatchSpeedConfirmModal from "./components/BatchSpeedConfirmModal.vue";
import GroupEditModal from "./components/GroupEditModal.vue";
import RegionManageModal from "./components/RegionManageModal.vue";

// Hooks
import { useProxyGroups } from "./hooks/useProxyGroups";
import { useNodeFilter } from "./hooks/useNodeFilter";
import { useSpeedtestActions } from "./hooks/useSpeedtestActions";
import { useRegionRules } from "./hooks/useRegionRules";

// Store 实例（用于模板直接读取批量测速进度等全局状态）
const proxyStore = useProxyStore();
const speedtestStore = useSpeedtestStore();
const toast = useToast();

// ==================== Hook 初始化 ====================

const {
  groups, loading, selectedGroupTag,
  systemGroups, regionGroups,
  isSelectorGroup, routingGroupTags, rawNodes,
  initSelectedGroup, handleGroupSelect,
} = useProxyGroups();

const {
  searchText, sortConfig, sortLabels,
  displayNodes, cycleSortKey, toggleSortOrder, clearSearch,
} = useNodeFilter(rawNodes);

const {
  showConfirmModal, batchEstimate,
  handleNodeSelect, handleRunLatency,
  handleSingleLatency, handleSingleSpeed,
  confirmBatchSpeedTest,
} = useSpeedtestActions({
  selectedGroupTag,
  rawNodes,
  isSelectorGroup,
});

const {
  showRegionModal,
  openRegionModal,
  closeRegionModal,
} = useRegionRules();

// ==================== 视图模式 (Grid 网格 / List 列表) ====================
const viewMode = ref<"grid" | "list">(
  (localStorage.getItem("auroweave_proxies_view_mode") as "grid" | "list") || "grid"
);

function handleToggleViewMode(mode: "grid" | "list") {
  viewMode.value = mode;
  localStorage.setItem("auroweave_proxies_view_mode", mode);
}

// ==================== 分组配置编辑（逻辑简单，内联管理） ====================

const showGroupEditModal = ref(false);
const editingGroupTag = ref("");
const editingGroupType = ref("");
const editingGroupConfig = ref<Record<string, any>>({});

/** 打开分组配置编辑弹窗 */
function openGroupEdit(groupTag: string) {
  const group = groups.value.find((g) => g.tag === groupTag);
  if (!group) return;
  editingGroupTag.value = groupTag;
  editingGroupType.value = group.type;
  editingGroupConfig.value = {
    interval: (group as any).interval || "3m",
    tolerance: (group as any).tolerance || 50,
    url: (group as any).url || "http://www.gstatic.com/generate_204",
  };
  showGroupEditModal.value = true;
}


/** 保存分组配置（调用 IPC 持久化，失败不关闭弹窗） */
async function saveGroupConfig() {
  const tag = editingGroupTag.value;
  if (!tag) return;
  const cfg = editingGroupConfig.value;
  try {
    const res = await proxyStore.updateGroupConfig(tag, {
      interval: cfg.interval,
      tolerance: cfg.tolerance,
      url: cfg.url,
    });
    if (res.success) {
      toast.success("分组配置已更新", "将在下次重建内核配置时生效");
      showGroupEditModal.value = false;
    } else {
      toast.error("保存分组配置失败", res.error || "未知错误");
    }
  } catch (e) {
    toast.error("保存分组配置失败", e instanceof Error ? e.message : String(e));
  }
}

// ==================== 事件协调 ====================

/** 分组切换：清除搜索 + 调用 Hook */
async function onGroupSelect(groupTag: string) {
  clearSearch();
  await handleGroupSelect(groupTag);
}

// ==================== 生命周期 ====================

/** 定期刷新分组状态（更新 URLTest 组的 now 字段）的定时器 */
let groupRefreshTimer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  proxyStore.loadCustomGroupRules();
});

// KeepAlive 激活时：重新拉取分组与节点数据
// 解决订阅刷新后 nodeMap 被清空但 ProxiesView 未重新挂载导致节点不显示的问题
onActivated(async () => {
  await proxyStore.fetchGroups();
  await speedtestStore.init();
  await initSelectedGroup();

  // 定期刷新分组列表以更新 URLTest 组的 now 字段（当前选中节点）
  if (groupRefreshTimer) clearInterval(groupRefreshTimer);
  groupRefreshTimer = setInterval(async () => {
    // 静默刷新：不触发 loading 状态，避免 UI 闪烁
    await proxyStore.refreshGroups();
  }, 15000);
});

onDeactivated(() => {
  if (groupRefreshTimer) {
    clearInterval(groupRefreshTimer);
    groupRefreshTimer = null;
  }
});
</script>

<template>
  <div class="proxies-view">
    <!-- 批量测速进度条（全局浮层） -->
    <div
      v-if="speedtestStore.isBatchTesting && speedtestStore.batchProgress"
      class="batch-progress-card glass-effect"
    >
      <div class="progress-info">
        <span>正在测速: <strong>{{ speedtestStore.batchProgress.current_node }}</strong></span>
        <span>进度: {{ speedtestStore.batchProgress.current_index }} / {{ speedtestStore.batchProgress.total }}</span>
      </div>
      <div class="progress-bar-bg">
        <div
          class="progress-bar-fill"
          :style="{ width: `${(speedtestStore.batchProgress.current_index / speedtestStore.batchProgress.total) * 100}%` }"
        ></div>
      </div>
      <button class="btn-cancel" @click="speedtestStore.cancelBatch">取消测速</button>
    </div>

    <!-- 双栏布局区域 -->
    <div v-if="groups.length > 0" class="proxies-layout">
      <!-- 左栏：分组选择器 -->
      <GroupSidebar
        :system-groups="systemGroups"
        :region-groups="regionGroups"
        :recent-groups="proxyStore.recentGroups"
        :selected-group-tag="selectedGroupTag"
        :routing-group-tags="routingGroupTags"
        @select="onGroupSelect"
        @edit-group="openGroupEdit"
        @manage-regions="openRegionModal"
      />

      <!-- 右栏：节点内容区 -->
      <main class="nodes-content glass-effect">
        <!-- 工具栏 -->
        <NodeToolbar
          :group-tag="selectedGroupTag"
          :node-count="rawNodes.length"
          :filtered-count="displayNodes.length"
          :is-filtering="!!searchText.trim()"
          :search-text="searchText"
          :sort-config="sortConfig"
          :sort-labels="sortLabels"
          :view-mode="viewMode"
          :is-testing-latency="speedtestStore.isTestingLatency"
          @update:search-text="searchText = $event"
          @cycle-sort="cycleSortKey"
          @toggle-sort-order="toggleSortOrder"
          @toggle-view-mode="handleToggleViewMode"
          @run-latency="handleRunLatency"
          @show-batch-modal="showConfirmModal = true"
          @refresh="proxyStore.fetchGroups"
        />

        <!-- 节点列表 -->
        <NodeListPanel
          :nodes="displayNodes"
          :raw-count="rawNodes.length"
          :loading="loading"
          :search-text="searchText"
          :is-selectable="isSelectorGroup"
          :layout-mode="viewMode"
          :fetch-error="proxyStore.error"
          @select="handleNodeSelect"
          @test-latency="handleSingleLatency"
          @test-speed="handleSingleSpeed"
        />
      </main>
    </div>

    <!-- 异常或空状态 -->
    <div v-else class="state-tip-full">
      <SvgIcon name="proxies" :size="48" class="empty-icon" />
      <p>未检测到运行中的代理节点组。</p>
      <p class="sub-tip">请确保 Sing-box 后台核心已成功启动且配置导入正确。</p>
      <button class="btn-retry" @click="proxyStore.fetchGroups">
        <SvgIcon name="refresh" :size="12" class="icon-gap" />
        重试刷新
      </button>
    </div>

    <!-- 弹窗区 -->
    <BatchSpeedConfirmModal
      :visible="showConfirmModal"
      :group-tag="selectedGroupTag"
      :node-count="batchEstimate.count"
      :estimate-minutes="batchEstimate.minutes"
      :estimate-mb="batchEstimate.mb"
      @close="showConfirmModal = false"
      @confirm="confirmBatchSpeedTest"
    />

    <GroupEditModal
      :visible="showGroupEditModal"
      :group-tag="editingGroupTag"
      :group-type="editingGroupType"
      :config="editingGroupConfig"
      @close="showGroupEditModal = false"
      @save="saveGroupConfig"
    />

    <RegionManageModal
      :visible="showRegionModal"
      @close="closeRegionModal"
    />
  </div>
</template>

<style scoped>
.proxies-view {
  padding: var(--space-5);
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.proxies-layout {
  display: flex;
  gap: var(--space-4);
  height: 100%;
  overflow: hidden;
}

/* 右栏：节点内容区 */
.nodes-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: var(--space-5);
  overflow: hidden;
}

/* 批量测速进度条 */
.batch-progress-card {
  padding: var(--space-3) var(--space-4);
  margin-bottom: var(--space-3);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex-shrink: 0;
}

.btn-cancel {
  align-self: flex-end;
  background: transparent;
  border: none;
  color: var(--accent-red);
  font-size: var(--text-xs);
  cursor: pointer;
}

/* 异常与空状态 */
.state-tip-full {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  gap: var(--space-3);
}

.empty-icon {
  color: var(--text-tertiary);
}

.sub-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.btn-retry {
  display: flex;
  align-items: center;
  padding: var(--space-2) var(--space-4);
  background: var(--accent-blue-glow);
  border: 1px solid var(--accent-blue);
  border-radius: var(--radius-sm);
  color: var(--accent-blue);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-retry:hover {
  background: var(--accent-blue);
  color: var(--text-on-accent);
}

.icon-gap {
  margin-right: 4px;
}
</style>
