<script setup lang="ts">
/**
 * 订阅中心 (一级核心视图)
 * 作者: TanXiang
 *
 * 功能特色：
 * - 订阅源全景卡片流展示（支持 Clash / Sing-box / V2Ray 多协议格式）
 * - 流量与账户看板（已用/总量极光进度条、剩余天数、临期预警）
 * - 多源导入（URL 远程导入 / 本地配置文件 / 剪贴板一键聚合）
 * - 订阅属性编辑与高级配置（自定义 UA、定时更新周期）
 * - 单个快速切换、刷新、编辑、删除与一键批量安全更新
 */
import { ref, computed, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useToast } from "@/composables/useToast";
import { useConfirm } from "@/composables/useConfirm";
import { formatBytes } from "@/utils/format";
import BaseIcon from "@/components/common/BaseIcon.vue";
import SubscriptionImportModal from "./components/SubscriptionImportModal.vue";
import SubscriptionEditModal from "./components/SubscriptionEditModal.vue";
import SubscriptionInspectModal from "./components/SubscriptionInspectModal.vue";
import type { Subscription } from "@/types";

const subStore = useSubscriptionStore();
const toast = useToast();

const { subscriptions } = storeToRefs(subStore);

/** 正在操作中的订阅 ID 集合（支持多卡片同时显示 spinner） */
const operatingIds = ref<Set<string>>(new Set());
const batchUpdating = ref(false);
/** 批量更新进度：正在更新第 n 个 / 共 total 个（仅计远程订阅） */
const batchProgress = ref<{ current: number; total: number } | null>(null);
const showImportModal = ref(false);
const showEditModal = ref(false);
const showInspectModal = ref(false);
const currentEditingSub = ref<Subscription | null>(null);
const currentInspectSub = ref<Subscription | null>(null);

/** 标记某订阅为操作中 */
function markOperating(id: string) {
  operatingIds.value = new Set(operatingIds.value).add(id);
}

/** 解除某订阅的操作中状态 */
function unmarkOperating(id: string) {
  const next = new Set(operatingIds.value);
  next.delete(id);
  operatingIds.value = next;
}

function handleOpenInspect(sub: Subscription) {
  currentInspectSub.value = sub;
  showInspectModal.value = true;
}

onMounted(async () => {
  await subStore.fetchAll();
});

/** 统计总节点数 */
const totalNodeCount = computed(() => {
  return subscriptions.value.reduce((acc, cur) => acc + (cur.node_count || 0), 0);
});

/** 当前活跃订阅（多订阅聚合：取第一个活跃项用于标题展示） */
const activeSub = computed(() => {
  return subscriptions.value.find((s) => s.is_active) || null;
});

/** 活跃订阅数量（多订阅聚合语义） */
const activeCount = computed(() => {
  return subscriptions.value.filter((s) => s.is_active).length;
});

/** 格式化更新时间 */
function formatTime(ts?: number | null): string {
  if (!ts) return "从未更新";
  const d = new Date(ts);
  const now = new Date();
  const diffMin = Math.floor((now.getTime() - d.getTime()) / 60000);
  if (diffMin < 1) return "刚刚更新";
  if (diffMin < 60) return `${diffMin} 分钟前`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `${diffHours} 小时前`;
  return `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** 计算流量百分比 */
function getTrafficPercentage(sub: Subscription): number {
  if (!sub.user_info || !sub.user_info.total_bytes || sub.user_info.total_bytes <= 0) {
    return 0;
  }
  const used = (sub.user_info.upload_bytes || 0) + (sub.user_info.download_bytes || 0);
  const pct = Math.round((used / sub.user_info.total_bytes) * 100);
  return Math.min(100, Math.max(0, pct));
}

/** 计算剩余天数 */
function getRemainingDays(expireTimestamp?: number | null): string {
  if (!expireTimestamp || expireTimestamp <= 0) return "长期有效";
  const nowSec = Math.floor(Date.now() / 1000);
  const diffSec = expireTimestamp - nowSec;
  if (diffSec <= 0) return "已到期";
  const days = Math.ceil(diffSec / 86400);
  return `剩余 ${days} 天`;
}

/** 获取来源标签文案与图标 */
function getSourceBadge(sub: Subscription) {
  if (sub.source_type === "clipboard") {
    return { label: "剪贴板", icon: "ClipboardList" };
  }
  if (sub.source_type === "local_file") {
    return { label: "本地文件", icon: "FileUp" };
  }
  return { label: (sub.format || "URL").toUpperCase(), icon: "Globe" };
}

/** 切换订阅聚合开关（多订阅语义：已激活→移出聚合，未激活→加入聚合） */
async function handleActivate(id: string) {
  const target = subscriptions.value.find((s) => s.id === id);
  const wasActive = target?.is_active ?? false;
  markOperating(id);
  try {
    const res = await subStore.activateSub(id);
    if (res.success) {
      if (wasActive) {
        toast.success("已移出聚合", "节点列表已重建，其他活跃订阅不受影响");
      } else {
        toast.success("已加入聚合", `当前共 ${activeCount.value} 个订阅参与节点聚合`);
      }
    } else {
      toast.error(wasActive ? "移出失败" : "加入失败", res.error || "配置应用异常");
    }
  } finally {
    unmarkOperating(id);
  }
}

/** 刷新单个订阅 */
async function handleRefresh(id: string) {
  markOperating(id);
  try {
    const res = await subStore.refreshSub(id);
    if (res.success) {
      toast.success("订阅刷新成功", `当前节点数: ${res.data?.node_count || 0}`);
    } else {
      toast.error("刷新失败", res.error || "网络拉取超时，已保留原配置");
    }
  } finally {
    unmarkOperating(id);
  }
}

/** 一键更新全部订阅 */
async function handleBatchUpdateAll() {
  if (subscriptions.value.length === 0 || batchUpdating.value) return;
  batchUpdating.value = true;
  let successCount = 0;
  let failCount = 0;

  // 仅远程订阅需要网络刷新，剪贴板订阅跳过
  const remoteSubs = subscriptions.value.filter((s) => s.source_type !== "clipboard");
  batchProgress.value = { current: 0, total: remoteSubs.length };

  try {
    for (const sub of remoteSubs) {
      batchProgress.value = { current: successCount + failCount + 1, total: remoteSubs.length };
      markOperating(sub.id);
      try {
        const res = await subStore.refreshSub(sub.id);
        if (res.success) {
          successCount++;
        } else {
          failCount++;
        }
      } finally {
        unmarkOperating(sub.id);
      }
    }
  } finally {
    batchUpdating.value = false;
    batchProgress.value = null;
  }

  if (failCount === 0) {
    toast.success("批量更新完成", `成功刷新全量 ${successCount} 个远程订阅`);
  } else {
    toast.warning("批量更新完毕", `成功: ${successCount}，失败: ${failCount} (已保留原节点)`);
  }
}

/** 打开编辑弹窗 */
function handleOpenEdit(sub: Subscription) {
  currentEditingSub.value = sub;
  showEditModal.value = true;
}

/** 删除订阅 */
async function handleDelete(sub: Subscription) {
  const confirmed = await useConfirm().ask({
    title: "删除订阅",
    message: `确定要删除订阅「${sub.name}」吗？此操作不可恢复。`,
    confirmText: "删除",
    level: "danger",
  });
  if (!confirmed) return;
  markOperating(sub.id);
  try {
    const res = await subStore.removeSub(sub.id);
    if (res.success) {
      toast.success("已删除订阅", sub.name);
    } else {
      toast.error("删除失败", res.error || "未知错误");
    }
  } finally {
    unmarkOperating(sub.id);
  }
}
</script>

<template>
  <div class="subscriptions-view-container">
    <!-- 顶部状态栏与操作工具条 -->
    <header class="subscriptions-header glass-effect">
      <div class="header-left">
        <div class="title-row">
          <div class="icon-orb">
            <BaseIcon name="Rss" :size="20" color="#00f2fe" />
          </div>
          <div>
            <h1 class="page-title">订阅中心</h1>
            <p class="page-subtitle">统一管理机场订阅源、流量消耗监控与全量节点池</p>
          </div>
        </div>

        <!-- 统计徽章群 -->
        <div class="stats-badges">
          <div class="stat-badge">
            <span class="badge-label">已导入订阅</span>
            <span class="badge-value">{{ subscriptions.length }}</span>
          </div>
          <div class="stat-badge">
            <span class="badge-label">节点总规模</span>
            <span class="badge-value cyan">{{ totalNodeCount }}</span>
          </div>
          <div class="stat-badge" v-if="activeSub">
            <span class="badge-label">当前生效</span>
            <span class="badge-value green" :title="activeSub.name">{{ activeSub.name }}</span>
          </div>
        </div>
      </div>

      <!-- 右侧操作栏 -->
      <div class="header-actions">
        <button
          class="btn-action primary"
          @click="showImportModal = true"
        >
          <BaseIcon name="PlusCircle" :size="14" />
          <span>导入新订阅</span>
        </button>

        <button
          class="btn-action secondary"
          :disabled="batchUpdating || subscriptions.length === 0"
          @click="handleBatchUpdateAll"
          title="并发拉取并更新所有远程订阅源"
        >
          <BaseIcon name="RefreshCw" :size="14" :class="{ spin: batchUpdating }" />
          <span>{{
            batchUpdating && batchProgress
              ? `正在更新 ${batchProgress.current}/${batchProgress.total}...`
              : batchUpdating
                ? "正在批量更新..."
                : "一键更新全部"
          }}</span>
        </button>
      </div>
    </header>

    <!-- 订阅卡片网格列表 -->
    <main class="subscriptions-main">
      <!-- 空状态 -->
      <div v-if="subscriptions.length === 0" class="empty-state-card glass-effect">
        <div class="empty-icon-wrap">
          <BaseIcon name="Rss" :size="48" color="rgba(0, 242, 254, 0.4)" />
        </div>
        <h3>尚未导入任何订阅</h3>
        <p>支持导入远程 URL、剪贴板节点集合以及本地 Clash YAML / sing-box JSON 文件</p>
        <button class="btn-import-empty" @click="showImportModal = true">
          <BaseIcon name="PlusCircle" :size="15" />
          <span>立即导入第一个订阅</span>
        </button>
      </div>

      <!-- 订阅卡片网格 -->
      <div v-else class="subscriptions-grid">
        <div
          v-for="sub in subscriptions"
          :key="sub.id"
          class="subscription-card glass-effect"
          :class="{ active: sub.is_active, operating: operatingIds.has(sub.id) }"
        >
          <!-- 卡片头部：名称、来源格式与状态 -->
          <div class="card-header">
            <div class="sub-title-group">
              <span class="sub-name" :title="sub.name">{{ sub.name }}</span>
              <span class="format-badge">
                <BaseIcon :name="getSourceBadge(sub).icon" :size="10" />
                <span>{{ getSourceBadge(sub).label }}</span>
              </span>
            </div>
            <div class="sub-status-group">
              <span v-if="sub.is_active" class="active-tag">
                <span class="pulse-dot"></span>
                当前生效
              </span>
              <span v-else class="standby-tag">备用</span>
            </div>
          </div>

          <!-- 卡片中部：流量极光进度条 (若存在 user_info) -->
          <div class="traffic-section" v-if="sub.user_info && sub.user_info.total_bytes">
            <div class="traffic-header">
              <span class="traffic-label">
                <BaseIcon name="Activity" :size="12" />
                已用 {{ formatBytes((sub.user_info.upload_bytes || 0) + (sub.user_info.download_bytes || 0)) }} / {{ formatBytes(sub.user_info.total_bytes) }}
              </span>
              <span class="traffic-percent">{{ getTrafficPercentage(sub) }}%</span>
            </div>
            <div class="progress-bar-track">
              <div
                class="progress-bar-fill"
                :style="{ width: `${getTrafficPercentage(sub)}%` }"
                :class="{
                  warning: getTrafficPercentage(sub) > 80 && getTrafficPercentage(sub) <= 90,
                  danger: getTrafficPercentage(sub) > 90
                }"
              ></div>
            </div>
            <div class="traffic-footer">
              <span class="expire-time">
                <BaseIcon name="Calendar" :size="11" />
                {{ getRemainingDays(sub.user_info.expire_timestamp) }}
              </span>
              <span class="traffic-subtext">
                ↑ {{ formatBytes(sub.user_info.upload_bytes ?? 0) }} · ↓ {{ formatBytes(sub.user_info.download_bytes ?? 0) }}
              </span>
            </div>
          </div>

          <!-- 默认基础元数据 -->
          <div class="card-meta-row" v-else>
            <div class="meta-item">
              <BaseIcon name="Radio" :size="13" />
              <span>{{ sub.node_count || 0 }} 个节点</span>
            </div>
            <div class="meta-item">
              <BaseIcon name="Clock" :size="13" />
              <span>{{ formatTime(sub.last_updated) }}</span>
            </div>
          </div>

          <!-- 卡片底部操作栏 -->
          <div class="card-footer">
            <div class="meta-brief" v-if="sub.user_info">
              <BaseIcon name="Radio" :size="12" />
              <span>{{ sub.node_count || 0 }} 节点 · {{ formatTime(sub.last_updated) }}</span>
            </div>
            <div class="sub-url-hint" v-else :title="sub.url">
              {{ sub.url }}
            </div>

            <div class="action-buttons">
              <!-- 聚合开关按钮（多订阅：未激活→加入聚合，已激活→移出聚合） -->
              <button
                v-if="!sub.is_active"
                class="btn-card-action activate"
                :disabled="operatingIds.has(sub.id)"
                @click="handleActivate(sub.id)"
                title="加入节点聚合（与其他活跃订阅合并生成配置）"
              >
                <BaseIcon name="Check" :size="13" />
                <span>加入聚合</span>
              </button>
              <button
                v-else
                class="btn-card-action deactivate"
                :disabled="operatingIds.has(sub.id)"
                @click="handleActivate(sub.id)"
                title="从节点聚合中移除（其他活跃订阅保留）"
              >
                <BaseIcon name="X" :size="13" />
                <span>移出聚合</span>
              </button>

              <!-- 刷新按钮 (仅远程订阅支持刷新) -->
              <button
                v-if="sub.source_type !== 'clipboard'"
                class="btn-card-action refresh"
                :disabled="operatingIds.has(sub.id)"
                @click="handleRefresh(sub.id)"
                title="从云端拉取更新"
              >
                <BaseIcon name="RefreshCw" :size="13" :class="{ spin: operatingIds.has(sub.id) }" />
              </button>

              <!-- 查看完整配置按钮 -->
              <button
                class="btn-card-action inspect"
                @click="handleOpenInspect(sub)"
                title="查看清洗前/后完整配置与节点详情"
              >
                <BaseIcon name="Code" :size="13" />
              </button>

              <!-- 编辑按钮 -->
              <button
                class="btn-card-action edit"
                @click="handleOpenEdit(sub)"
                title="编辑订阅属性与自定义 UA"
              >
                <BaseIcon name="Settings2" :size="13" />
              </button>

              <!-- 删除按钮 -->
              <button
                class="btn-card-action delete"
                :disabled="operatingIds.has(sub.id)"
                @click="handleDelete(sub)"
                title="删除此订阅"
              >
                <BaseIcon name="Trash2" :size="13" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </main>

    <!-- 导入模态框 -->
    <SubscriptionImportModal
      v-model:visible="showImportModal"
      @success="subStore.fetchAll"
    />

    <!-- 编辑模态框 -->
    <SubscriptionEditModal
      v-model:visible="showEditModal"
      :subscription="currentEditingSub"
      @success="subStore.fetchAll"
    />

    <!-- 订阅配置深度查看器模态框 (全平台多端适配) -->
    <SubscriptionInspectModal
      v-model:visible="showInspectModal"
      :subscription="currentInspectSub"
    />
  </div>
</template>

<style scoped>
.subscriptions-view-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 16px 20px;
  gap: 16px;
  overflow: hidden;
  box-sizing: border-box;
}

/* 顶部导航 Header */
.subscriptions-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 18px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.08);
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 24px;
}

.title-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.icon-orb {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: radial-gradient(circle at center, rgba(0, 242, 254, 0.2) 0%, rgba(0, 0, 0, 0) 70%);
  border: 1px solid rgba(0, 242, 254, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
}

.page-title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: #fff;
  letter-spacing: 0.5px;
}

.page-subtitle {
  margin: 2px 0 0 0;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
}

.stats-badges {
  display: flex;
  align-items: center;
  gap: 8px;
}

.stat-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
}

.badge-label {
  color: rgba(255, 255, 255, 0.4);
}

.badge-value {
  font-weight: 600;
  color: #fff;
}

.badge-value.cyan {
  color: #00f2fe;
}

.badge-value.green {
  color: #43e97b;
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.btn-action {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  border: 1px solid transparent;
}

.btn-action.primary {
  background: linear-gradient(135deg, rgba(0, 242, 254, 0.85) 0%, rgba(79, 172, 254, 0.85) 100%);
  color: #000;
  box-shadow: 0 4px 12px rgba(0, 242, 254, 0.2);
}

.btn-action.primary:hover {
  filter: brightness(1.1);
  transform: translateY(-1px);
}

.btn-action.secondary {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.85);
}

.btn-action.secondary:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.btn-action:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 主体网格卡片区 */
.subscriptions-main {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.subscriptions-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  gap: 14px;
}

.subscription-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.07);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  position: relative;
}

.subscription-card:hover {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.15);
  transform: translateY(-2px);
}

.subscription-card.active {
  border-color: rgba(0, 242, 254, 0.4);
  background: radial-gradient(circle at top right, rgba(0, 242, 254, 0.06) 0%, rgba(255, 255, 255, 0.02) 100%);
  box-shadow: 0 6px 20px rgba(0, 242, 254, 0.08);
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.sub-title-group {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.sub-name {
  font-size: 14px;
  font-weight: 600;
  color: #fff;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.format-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 9.5px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.7);
  letter-spacing: 0.5px;
}

.sub-status-group {
  flex-shrink: 0;
}

.active-tag {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 600;
  color: #00f2fe;
  padding: 3px 8px;
  border-radius: 6px;
  background: rgba(0, 242, 254, 0.12);
  border: 1px solid rgba(0, 242, 254, 0.25);
}

.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #00f2fe;
  box-shadow: 0 0 8px #00f2fe;
  animation: pulseDot 2s infinite ease-in-out;
}

@keyframes pulseDot {
  0%, 100% { transform: scale(0.9); opacity: 0.7; }
  50% { transform: scale(1.3); opacity: 1; }
}

.standby-tag {
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.35);
}

/* 流量进度条区域 */
.traffic-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(255, 255, 255, 0.04);
}

.traffic-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 11px;
}

.traffic-label {
  display: flex;
  align-items: center;
  gap: 5px;
  color: rgba(255, 255, 255, 0.75);
  font-weight: 500;
}

.traffic-percent {
  font-weight: 700;
  color: #00f2fe;
}

.progress-bar-track {
  height: 5px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  border-radius: 3px;
  background: linear-gradient(90deg, #00f2fe 0%, #4facfe 100%);
  transition: width 0.4s ease;
}

.progress-bar-fill.warning {
  background: linear-gradient(90deg, #f6d365 0%, #fda085 100%);
}

.progress-bar-fill.danger {
  background: linear-gradient(90deg, #ff758c 0%, #ff7eb3 100%);
}

.traffic-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
}

.expire-time {
  display: flex;
  align-items: center;
  gap: 4px;
}

/* 无流量基础信息 */
.card-meta-row {
  display: flex;
  gap: 16px;
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.55);
}

.meta-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

/* 卡片底部操作栏 */
.card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-top: 6px;
  border-top: 1px solid rgba(255, 255, 255, 0.05);
}

.meta-brief {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.4);
}

.sub-url-hint {
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.3);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action-buttons {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-card-action {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 9px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.7);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-card-action:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.btn-card-action.activate {
  background: rgba(0, 242, 254, 0.1);
  border-color: rgba(0, 242, 254, 0.3);
  color: #00f2fe;
}

.btn-card-action.activate:hover:not(:disabled) {
  background: rgba(0, 242, 254, 0.2);
  color: #fff;
}

/* 移出聚合按钮：弱化的红调，区别于删除（红色实感） */
.btn-card-action.deactivate {
  background: rgba(255, 255, 255, 0.04);
  border-color: rgba(255, 255, 255, 0.15);
  color: var(--text-secondary);
}

.btn-card-action.deactivate:hover:not(:disabled) {
  background: rgba(255, 77, 79, 0.08);
  border-color: rgba(255, 77, 79, 0.25);
  color: #ff8f91;
}

.btn-card-action.delete:hover:not(:disabled) {
  background: rgba(255, 77, 79, 0.15);
  border-color: rgba(255, 77, 79, 0.3);
  color: #ff4d4f;
}

/* 空状态卡片 */
.empty-state-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px dashed rgba(255, 255, 255, 0.1);
  text-align: center;
}

.empty-icon-wrap {
  margin-bottom: 16px;
}

.empty-state-card h3 {
  margin: 0 0 6px 0;
  font-size: 16px;
  color: #fff;
}

.empty-state-card p {
  margin: 0 0 20px 0;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
}

.btn-import-empty {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 9px 18px;
  border-radius: 8px;
  background: linear-gradient(135deg, rgba(0, 242, 254, 0.9) 0%, rgba(79, 172, 254, 0.9) 100%);
  color: #000;
  font-size: 13px;
  font-weight: 600;
  border: none;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 4px 14px rgba(0, 242, 254, 0.25);
}

.btn-import-empty:hover {
  filter: brightness(1.1);
  transform: translateY(-1px);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
