/**
 * Pinia Store — AI 服务解锁检测
 * 作者: TanXiang
 *
 * 数据流与 speedtest.store 同构：unlockMap 按 nodeTag 映射最近一次检测结果，
 * 批量进度经 unlock-check-progress 事件流式回填，终止事件统一复位状态。
 */
import { defineStore } from "pinia";
import { ref } from "vue";
import type { UnlockCheckResult, UnlockServiceId, UnlockStatus } from "@/types";
import {
  runUnlockCheckSingle,
  runUnlockCheckBatch,
  cancelUnlockCheckBatch,
  getLatestUnlockResults,
  listenUnlockCheckProgress,
  type UnlockBatchProgressPayload,
} from "@/api/ipc/unlock";
import { useToast } from "@/composables/useToast";

/** 服务显示元数据（徽章字母 + 名称） */
export const UNLOCK_SERVICE_META: Record<
  UnlockServiceId,
  { badge: string; label: string }
> = {
  gemini: { badge: "G", label: "Gemini" },
  claude: { badge: "C", label: "Claude" },
  chatgpt: { badge: "O", label: "ChatGPT" },
};

/** 服务状态 → 徽章颜色 token */
export function unlockStatusColor(status?: UnlockStatus): string {
  switch (status) {
    case "yes":
      return "var(--accent-green)";
    case "no":
      return "var(--accent-red)";
    case "risky":
      return "var(--accent-orange)";
    default:
      return "var(--text-tertiary)"; // failed / 未测
  }
}

/** 解锁排序权重：yes=0 最优 → risky → no → failed → 未测最大 */
export function unlockSortWeight(
  services: Partial<Record<UnlockServiceId, UnlockStatus>> | undefined
): number {
  if (!services) return 900;
  const values = Object.values(services);
  if (values.length === 0) return 900;
  // 取全部服务状态中的最优档作为节点档位（任意服务可用即有排前价值）
  if (values.includes("yes")) return 0;
  if (values.includes("risky")) return 1;
  if (values.includes("no")) return 2;
  if (values.includes("failed")) return 3;
  return 900;
}

export const useUnlockStore = defineStore("unlock", () => {
  // nodeTag -> 最近一次检测结果
  const unlockMap = ref<Record<string, UnlockCheckResult>>({});
  // 正在单节点检测的节点 Tag
  const checkingNodes = ref<Set<string>>(new Set());

  // 批量检测状态
  const isBatchChecking = ref(false);
  const batchProgress = ref<UnlockBatchProgressPayload | null>(null);
  const batchCancelled = ref(false);

  let unlistenProgress: (() => void) | null = null;
  let initPromise: Promise<void> | null = null;

  /** 初始化：回填历史最近结果 + 注册进度监听（幂等） */
  async function init() {
    if (initPromise) return initPromise;
    initPromise = (async () => {
      const res = await getLatestUnlockResults();
      if (res.success && res.data) {
        const map: Record<string, UnlockCheckResult> = {};
        for (const rec of res.data) {
          map[rec.node_tag] = rec;
        }
        unlockMap.value = map;
      }

      if (!unlistenProgress) {
        unlistenProgress = await listenUnlockCheckProgress((payload) => {
          const isTerminal =
            payload.current_index === payload.total &&
            payload.total > 0 &&
            !payload.current_node;

          if (isTerminal) {
            isBatchChecking.value = false;
            batchCancelled.value = false;
            batchProgress.value = null;
            return;
          }

          if (batchCancelled.value) return;

          batchProgress.value = payload;
          isBatchChecking.value = payload.current_index <= payload.total;
          if (payload.result && payload.current_node) {
            unlockMap.value = {
              ...unlockMap.value,
              [payload.current_node]: payload.result,
            };
          }
        });
      }
    })().catch((e) => {
      initPromise = null;
      throw e;
    });
    return initPromise;
  }

  /** 单节点解锁检测 */
  async function checkSingle(nodeTag: string, withIp = true) {
    checkingNodes.value.add(nodeTag);
    checkingNodes.value = new Set(checkingNodes.value);
    try {
      const res = await runUnlockCheckSingle(nodeTag, [], withIp);
      if (res.success && res.data) {
        unlockMap.value = {
          ...unlockMap.value,
          [nodeTag]: res.data,
        };
      }
      return res;
    } finally {
      checkingNodes.value.delete(nodeTag);
      checkingNodes.value = new Set(checkingNodes.value);
    }
  }

  /** 启动批量解锁检测（fire-and-forget，进度经事件回填） */
  async function startBatchCheck(
    groupTag: string,
    nodeTags: string[],
    withIp = true
  ) {
    isBatchChecking.value = true;
    batchCancelled.value = false;
    // 立即铺一条 0/total 进度：test-core 按批（32 节点）探测，整批跑完才发首个
    // 事件——留空会让进度条在数十秒内完全缺席，取消按钮也就无从点起
    batchProgress.value = {
      current_index: 0,
      total: nodeTags.length,
      current_node: "正在准备检测...",
    };
    try {
      const res = await runUnlockCheckBatch(groupTag, nodeTags, [], withIp);
      if (!res.success) {
        isBatchChecking.value = false;
        batchProgress.value = null;
        useToast().error(
          "批量解锁检测启动失败",
          res.error ?? "请确认 Sing-box 核心是否在运行。"
        );
      }
      return res;
    } catch (e) {
      isBatchChecking.value = false;
      batchProgress.value = null;
      useToast().error("批量解锁检测启动失败", "与后端通信异常，请稍后重试。");
      throw e;
    }
  }

  /** 取消批量解锁检测 */
  async function cancelBatch() {
    batchCancelled.value = true;
    const res = await cancelUnlockCheckBatch();
    if (!res.success) {
      batchCancelled.value = false;
    }
    return res;
  }

  return {
    unlockMap,
    checkingNodes,
    isBatchChecking,
    batchProgress,
    batchCancelled,
    init,
    checkSingle,
    startBatchCheck,
    cancelBatch,
  };
});
