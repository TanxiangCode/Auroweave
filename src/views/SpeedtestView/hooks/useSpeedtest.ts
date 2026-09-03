/**
 * 测速操作 Hook
 * 作者: TanXiang
 *
 * 职责：节点选择、延迟测试、吞吐量测试、批量测速
 */
import { ref, computed } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";

/**
 * 测速操作 Hook
 */
export function useSpeedtest() {
  const proxyStore = useProxyStore();
  const speedtestStore = useSpeedtestStore();
  const toast = useToast();

  const activeGroupTag = ref<string>("");
  const showConfirmModal = ref(false);

  const currentNodes = computed(() => {
    const g = proxyStore.groups.find((x) => x.tag === activeGroupTag.value);
    return g ? g.proxies : [];
  });

  const activeGroupNow = computed(() => {
    const g = proxyStore.groups.find((x) => x.tag === activeGroupTag.value);
    return g?.now || "";
  });

  /** 初始化：拉取分组并选中第一个 */
  async function init() {
    await proxyStore.fetchGroups();
    await speedtestStore.init();
    if (proxyStore.groups.length > 0) {
      activeGroupTag.value = proxyStore.groups[0].tag;
    }
  }

  async function handleSelectNode(nodeTag: string) {
    if (!activeGroupTag.value) return;
    const res = await proxyStore.selectNode(activeGroupTag.value, nodeTag);
    if (res && res.success) {
      toast.success("节点已切换", `切至: ${nodeTag}`);
    } else {
      toast.error("切换节点失败", res?.error || "未知错误");
    }
  }

  async function handleRunLatency() {
    if (!activeGroupTag.value) return;
    // 过滤子策略组（selector/urltest/fallback 类型节点不是真实出口，无法测延迟）
    // group.proxies 仅是 tag 数组，需从 nodeMap 取节点对象获得 type
    const nodes = proxyStore.nodeMap.get(activeGroupTag.value) ?? [];
    const tags = nodes
      .filter((n) => !["selector", "urltest", "fallback"].includes(n.type.toLowerCase()))
      .map((n) => n.tag);
    if (tags.length === 0) {
      toast.warning("该策略组内没有可供测试的真实节点");
      return;
    }
    toast.info("正在并发测试延迟...");
    await speedtestStore.testLatency(activeGroupTag.value, tags);
    const success = Object.values(speedtestStore.latencyMap).filter((v) => v > 0).length;
    toast.success("延迟测试完成", `有效响应: ${success} / 总计: ${tags.length}`);
  }

  async function handleSingleLatency(nodeTag: string) {
    if (!activeGroupTag.value) return;
    await speedtestStore.testLatency(activeGroupTag.value, [nodeTag]);
  }

  async function handleSingleSpeed(nodeTag: string) {
    toast.info("开始节点吞吐量测试", `正在测试: ${nodeTag}`);
    const res = await speedtestStore.testSingleThroughput(nodeTag);
    if (res.success && res.data) {
      const mbps = (res.data.download_bps / (1024 * 1024)).toFixed(1);
      toast.success("单节点测速完成", `${nodeTag}: ${mbps} MB/s`);
    } else {
      toast.error("测速失败", res.error);
    }
  }

  async function confirmBatchSpeedTest() {
    showConfirmModal.value = false;
    if (!activeGroupTag.value) return;
    await speedtestStore.startBatchTest(activeGroupTag.value, currentNodes.value);
    toast.info("已启动批量串行测速任务");
  }

  return {
    activeGroupTag,
    showConfirmModal,
    currentNodes,
    activeGroupNow,
    init,
    handleSelectNode,
    handleRunLatency,
    handleSingleLatency,
    handleSingleSpeed,
    confirmBatchSpeedTest,
    proxyStore,
  };
}
