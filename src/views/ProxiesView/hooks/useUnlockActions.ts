/**
 * 解锁检测操作 Hook
 * 作者: TanXiang
 *
 * 职责：管理单节点解锁检测、批量解锁检测确认与启动
 * 与 useSpeedtestActions 同构，均含"临时切换出口"语义
 */
import { ref, computed, type Ref, type ComputedRef } from "vue";
import { useUnlockStore } from "@/stores/unlock.store";
import { useToast } from "@/composables/useToast";
import type { ProxyNode } from "@/types";

interface UseUnlockActionsOptions {
  selectedGroupTag: Ref<string>;
  rawNodes: ComputedRef<ProxyNode[]>;
}

/** 单节点检测时长上限（三服务串行 15s/请求 + ip-api 8s） */
const SINGLE_CHECK_TIMEOUT_MS = 60_000;

/** 批量预估：test-core 并发 8 路探测，每节点三服务约 8-12s，取均值 10s/8 路 */
const BATCH_PER_NODE_SECONDS_AT_CONCURRENCY_1 = 10;
const DEFAULT_CONCURRENCY = 8;

export function useUnlockActions(options: UseUnlockActionsOptions) {
  const unlockStore = useUnlockStore();
  const toast = useToast();

  const { selectedGroupTag, rawNodes } = options;

  /** 批量解锁检测确认弹窗 */
  const showUnlockModal = ref(false);

  /** 单节点解锁检测（卡片按钮） */
  async function handleSingleUnlockCheck(nodeTag: string) {
    toast.info("开始解锁检测", `正在检测: ${nodeTag}（将临时切换出口，结束后还原）`);
    try {
      const res = await unlockStore.checkSingle(nodeTag);
      if (res.success && res.data) {
        const svc = res.data.services ?? {};
        const summary = Object.entries(svc)
          .map(([k, v]) => `${k}: ${v}`)
          .join(" · ");
        toast.success(
          "解锁检测完成",
          `${nodeTag} — ${summary || "无结果"}${res.data.country_code ? ` · 出口 ${res.data.country_code}` : ""}`
        );
      } else {
        toast.error("解锁检测失败", res.error || "未知错误");
      }
    } catch (e) {
      toast.error("解锁检测失败", e instanceof Error ? e.message : String(e));
    }
  }

  /** 确认批量解锁检测 */
  async function confirmBatchUnlockCheck() {
    showUnlockModal.value = false;
    if (!selectedGroupTag.value) return;
    const tags = rawNodes.value
      .filter((n) => !["selector", "urltest", "fallback", "loadbalance"].includes(n.type.toLowerCase()))
      .map((n) => n.tag);
    if (tags.length === 0) {
      toast.warning("该策略组内没有可供检测的真实节点");
      return;
    }
    await unlockStore.startBatchCheck(selectedGroupTag.value, tags);
    toast.info("已启动批量解锁检测", `${tags.length} 个节点将逐个检测，完成后自动还原出口`);
  }

  /** 批量检测预估信息（并发 8 路：单节点耗时 / 并发数） */
  const unlockBatchEstimate = computed(() => {
    const count = rawNodes.value.length;
    const effectiveSeconds =
      Math.ceil(count / DEFAULT_CONCURRENCY) * BATCH_PER_NODE_SECONDS_AT_CONCURRENCY_1;
    return {
      count,
      minutes: Math.max(1, Math.ceil(effectiveSeconds / 60)),
    };
  });

  return {
    // 状态
    showUnlockModal,
    unlockBatchEstimate,
    // 方法
    handleSingleUnlockCheck,
    confirmBatchUnlockCheck,
    SINGLE_CHECK_TIMEOUT_MS,
  };
}
