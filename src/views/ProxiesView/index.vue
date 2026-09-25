<script setup lang="ts">
/**
 * 代理节点视图 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定、Hook 协调
 * 业务逻辑全部委托至各子组件与 Hook
 */
import { onMounted, onActivated, onDeactivated, ref, computed } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useUnlockStore } from "@/stores/unlock.store";
import { useToast } from "@/composables/useToast";
import SvgIcon from "@/components/common/SvgIcon.vue";
import BatchTaskDock from "@/components/speedtest/BatchTaskDock.vue";

// 子组件
import GroupSidebar from "./components/GroupSidebar.vue";
import NodeToolbar from "./components/NodeToolbar.vue";
import NodeListPanel from "./components/NodeListPanel.vue";
import BatchSpeedConfirmModal from "./components/BatchSpeedConfirmModal.vue";
import UnlockBatchConfirmModal from "./components/UnlockBatchConfirmModal.vue";
import GroupEditModal from "./components/GroupEditModal.vue";
import RegionManageModal from "./components/RegionManageModal.vue";

// Hooks
import { useProxyGroups } from "./hooks/useProxyGroups";
import { useNodeFilter } from "./hooks/useNodeFilter";
import { useSpeedtestActions } from "./hooks/useSpeedtestActions";
import { useUnlockActions } from "./hooks/useUnlockActions";
import { useRegionRules } from "./hooks/useRegionRules";

// Store 实例（用于模板直接读取批量测速进度等全局状态）
const proxyStore = useProxyStore();
const speedtestStore = useSpeedtestStore();
const unlockStore = useUnlockStore();
const toast = useToast();

// ==================== Hook 初始化 ====================

const {
  groups, loading, selectedGroupTag,
  systemGroups, regionGroups, customGroups,
  isSelectorGroup, routingGroupTags, rawNodes, currentGroup,
  initSelectedGroup, handleGroupSelect,
} = useProxyGroups();

const {
  searchText, sortConfig, sortLabels, unlockFilter, hideTimedOut,
  displayNodes, setSortConfig, clearSearch,
  pinnedSet, togglePinned,
} = useNodeFilter(rawNodes);

const {
  showConfirmModal, batchEstimate,
  handleNodeSelect, handleRunLatency,
  handleSingleLatency, handleSingleSpeed,
  confirmBatchSpeedTest,
} = useSpeedtestActions({
  selectedGroupTag,
  targetNodes: displayNodes,
  isSelectorGroup,
});

const {
  showUnlockModal, unlockBatchEstimate,
  handleSingleUnlockCheck, confirmBatchUnlockCheck,
} = useUnlockActions({
  selectedGroupTag,
  targetNodes: displayNodes,
});

const {
  showRegionModal,
  openRegionModal,
  closeRegionModal,
  editRuleByTag,
} = useRegionRules();

/** 含 unlock 匹配的规则名集合（GroupSidebar 时效提示用） */
const unlockRuleNames = computed(() =>
  proxyStore.customGroupRules
    .filter((r) => r.enabled && r.match_type === "unlock")
    .map((r) => r.name)
);

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
      toast.success("分组配置已更新", "配置已写入，重启内核后生效");
      showGroupEditModal.value = false;
    } else {
      toast.error("保存分组配置失败", res.error || "未知错误");
    }
  } catch (e) {
    toast.error("保存分组配置失败", e instanceof Error ? e.message : String(e));
  }
}

// ==================== 定位当前节点 ====================

/** 当前分组内被选中的节点 tag（selector 组的 now / urltest 组的 now） */
const activeNodeTag = computed(() => currentGroup.value?.now ?? "");

/** 当前分组内是否有选中节点（定位按钮可用性） */
const hasActiveNode = computed(() => {
  if (!activeNodeTag.value) return false;
  // 选中节点须在展示列表中（可能被搜索/筛选/订阅刷新移出）
  return displayNodes.value.some((n) => n.tag === activeNodeTag.value);
});

/** 定位：滚动到当前选中节点卡片并高亮闪烁一拍 */
function handleLocateActive() {
  const tag = activeNodeTag.value;
  if (!tag) return;
  // NodeListPanel 内卡片带 data-node-tag 标记；grid 布局的
  // content-visibility 不影响 offsetParent 定位
  const el = document.querySelector<HTMLElement>(`[data-node-tag="${CSS.escape(tag)}"]`);
  if (!el) {
    if (!hasActiveNode.value) {
      toast.warning("当前节点不在展示列表中", "可能已被搜索或筛选条件排除，试试清除过滤");
    }
    return;
  }
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  const card = el.querySelector<HTMLElement>(".node-card") ?? el;
  card.classList.add("locate-flash");
  setTimeout(() => card.classList.remove("locate-flash"), 1200);
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
// 性能：原来五个 await 串行（mode→groups→speedtest→unlock→nodes），
// 每次进入页面都叠加多轮 IPC 往返，路由切换动画期间阻塞渲染出现明显卡顿。
// 现在：分组数据先行（渲染依赖它），测速/解锁 store 初始化与节点拉取并行，
// 未完成的初始化不阻塞视图出现。
onActivated(async () => {
  // 事件监听注册型初始化（幂等，已有缓存直接返回）——后台并行，不阻塞渲染
  const storesInit = Promise.all([
    speedtestStore.init(),
    unlockStore.init(),
  ]).catch(() => {});

  await proxyStore.fetchGroups();
  await initSelectedGroup();
  await storesInit;

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
    <!-- 页面标题由全局顶栏 routeTitle 提供（Windows 无边框自绘标题栏同样常驻显示），
         内容区不再重复渲染标题与副标题 -->

    <!-- 双栏布局区域 -->
    <div v-if="groups.length > 0" class="proxies-layout">
      <!-- 左栏：分组选择器 -->
      <GroupSidebar
        :system-groups="systemGroups"
        :region-groups="regionGroups"
        :custom-groups="customGroups"
        :custom-rules-count="proxyStore.customGroupRules.length"
        :unlock-rule-names="unlockRuleNames"
        :recent-groups="proxyStore.recentGroups"
        :selected-group-tag="selectedGroupTag"
        :routing-group-tags="routingGroupTags"
        @select="onGroupSelect"
        @edit-group="openGroupEdit"
        @manage-regions="openRegionModal"
        @edit-rule="editRuleByTag"
      />

      <!-- 右栏：节点内容区 -->
      <main class="nodes-content glass-effect">
        <!-- 工具栏 -->
        <NodeToolbar
          :group-tag="selectedGroupTag"
          :node-count="rawNodes.length"
          :filtered-count="displayNodes.length"
          :is-filtering="!!searchText.trim() || !!unlockFilter || hideTimedOut"
          :search-text="searchText"
          :sort-config="sortConfig"
          :sort-labels="sortLabels"
          :view-mode="viewMode"
          :is-testing-latency="speedtestStore.isTestingLatency"
          :is-batch-speed-testing="speedtestStore.isBatchTesting"
          :is-unlock-checking="unlockStore.isBatchChecking"
          :unlock-filter="unlockFilter"
          :hide-timed-out="hideTimedOut"
          :has-active-node="hasActiveNode"
          @update:search-text="searchText = $event"
          @update:unlock-filter="unlockFilter = $event"
          @update:hide-timed-out="hideTimedOut = $event"
          @update-sort="setSortConfig"
          @toggle-view-mode="handleToggleViewMode"
          @run-latency="handleRunLatency"
          @show-batch-modal="showConfirmModal = true"
          @show-unlock-modal="showUnlockModal = true"
          @locate-active="handleLocateActive"
          @refresh="proxyStore.fetchGroups"
        />

        <!-- 节点列表 -->
        <NodeListPanel
          :nodes="displayNodes"
          :raw-count="rawNodes.length"
          :loading="loading"
          :search-text="searchText"
          :unlock-filter="unlockFilter"
          :hide-timed-out="hideTimedOut"
          :is-selectable="isSelectorGroup"
          :layout-mode="viewMode"
          :fetch-error="proxyStore.error"
          :pinned-set="pinnedSet"
          @select="handleNodeSelect"
          @test-latency="handleSingleLatency"
          @test-speed="handleSingleSpeed"
          @check-unlock="handleSingleUnlockCheck"
          @toggle-pin="togglePinned"
          @refresh-groups="proxyStore.fetchGroups"
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

    <!-- 批量任务聚合吸底 Dock（自适应横向并排，支持测延迟、测速与解锁检测，总高度锁定） -->
    <BatchTaskDock />

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

    <UnlockBatchConfirmModal
      :visible="showUnlockModal"
      :group-tag="selectedGroupTag"
      :node-count="unlockBatchEstimate.count"
      :estimate-minutes="unlockBatchEstimate.minutes"
      @close="showUnlockModal = false"
      @confirm="confirmBatchUnlockCheck"
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
  /* flex:1 填满剩余高度；批量进度条出现在底部时自动让出空间
     （原 height:100% 会把进度条挤出视口） */
  flex: 1;
  min-height: 0;
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
