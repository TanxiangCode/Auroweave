/**
 * 测速操作 Hook
 * 作者: TanXiang
 *
 * 职责：管理延迟测试、吞吐量测试、批量测速等操作
 */
import { ref, computed, type Ref, type ComputedRef } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";
import { bytesToMB } from "@/utils/format";
import type { ProxyNode } from "@/types";
import { getLatencyTestNodeTags } from "../utils/proxy-page";

interface UseSpeedtestActionsOptions {
  selectedGroupTag: Ref<string>;
  rawNodes: ComputedRef<ProxyNode[]>;
  isSelectorGroup: ComputedRef<boolean>;
}

/**
 * 测速操作 Hook
 *
 * @param options - 依赖注入：分组标签、原始节点、是否可选择
 */
export function useSpeedtestActions(options: UseSpeedtestActionsOptions) {
  const proxyStore = useProxyStore();
  const speedtestStore = useSpeedtestStore();
  const toast = useToast();

  const { selectedGroupTag, rawNodes, isSelectorGroup } = options;

  /** 批量测速确认弹窗 */
  const showConfirmModal = ref(false);

  /** 选择节点（切换出口） */
  async function handleNodeSelect(nodeTag: string) {
    if (!selectedGroupTag.value) return;
    if (!isSelectorGroup.value) {
      toast.warning("不支持切换", "该策略组为自动或非手动选择类型，无法手动指定节点。");
      return;
    }
    const res = await proxyStore.selectNode(selectedGroupTag.value, nodeTag);
    if (res && res.success) {
      toast.success("节点已切换", `当前出站: ${nodeTag}`);
    } else {
      toast.error("切换节点失败", res?.error || "未知错误");
    }
  }

  /** 批量延迟测试 */
  async function handleRunLatency() {
    if (!selectedGroupTag.value) return;
    toast.info("正在并发测试延迟...");
    // rawNodes 对真实组来自 nodeMap，对 custom:* 虚拟组来自 proxy 主组节点池本地匹配。
    // 虚拟组不会拥有 nodeMap[custom:*]，直接读 nodeMap 会把有节点的虚拟组误判为空。
    const tags = getLatencyTestNodeTags(rawNodes.value);

    if (tags.length === 0) {
      toast.warning("该策略组内没有可供测试的真实节点");
      return;
    }

    await speedtestStore.testLatency(selectedGroupTag.value, tags);
    
    // 刷新真实策略组状态；虚拟 custom:* 不存在于内核，无需发无效 IPC。
    await proxyStore.refreshGroups();
    if (!selectedGroupTag.value.startsWith("custom:")) {
      await proxyStore.fetchGroupNodes(selectedGroupTag.value);
    }

    const success = tags.filter(
      (tag) => (speedtestStore.latencyMap[tag] ?? 0) > 0
    ).length;
    toast.success("延迟测试完成", `有效响应: ${success} / 总计: ${tags.length}`);
  }

  /** 单节点延迟测试 */
  async function handleSingleLatency(nodeTag: string) {
    if (!selectedGroupTag.value) return;
    await speedtestStore.testLatency(selectedGroupTag.value, [nodeTag]);
    await proxyStore.refreshGroups();
  }

  /** 单节点吞吐量测试 */
  async function handleSingleSpeed(nodeTag: string) {
    toast.info("开始节点吞吐量测试", `正在测试: ${nodeTag}`);
    const res = await speedtestStore.testSingleThroughput(nodeTag);
    if (res.success && res.data) {
      const mbps = bytesToMB(res.data.download_bps);
      toast.success("单节点测速完成", `${nodeTag}: ${mbps} MB/s`);
    } else {
      toast.error("测速失败", res.error);
    }
  }

  /** 确认批量测速 */
  async function confirmBatchSpeedTest() {
    showConfirmModal.value = false;
    if (!selectedGroupTag.value) return;
    const tags = rawNodes.value.map((n) => n.tag);
    if (tags.length === 0) {
      toast.warning("该策略组内没有可测速的节点");
      return;
    }
    await speedtestStore.startBatchTest(selectedGroupTag.value, tags);
    toast.info("已启动批量串行测速任务");
  }

  /** 批量测速预估信息 */
  const batchEstimate = computed(() => {
    const count = rawNodes.value.length;
    const minutes = Math.ceil(
      (count * speedtestStore.THROUGHPUT_TEST_DURATION_SEC) / 60
    );
    const mb = Math.ceil(
      count * (speedtestStore.THROUGHPUT_TEST_CHUNK_BYTES / (1024 * 1024))
    );
    return { count, minutes, mb };
  });

  return {
    // 状态
    showConfirmModal,
    batchEstimate,
    // 方法
    handleNodeSelect,
    handleRunLatency,
    handleSingleLatency,
    handleSingleSpeed,
    confirmBatchSpeedTest,
  };
}
