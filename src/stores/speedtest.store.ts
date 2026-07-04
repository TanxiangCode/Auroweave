/**
 * Pinia Store — 智能测速
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref } from "vue";
import type { ThroughputResult } from "@/types";
import {
  runLatencyTest,
  runSingleThroughputTest,
  runBatchSpeedTest,
  cancelBatchSpeedTest,
  getSpeedTestResults,
  listenSpeedTestProgress,
  type BatchProgressPayload,
} from "@/api/ipc/speedtest";

export const useSpeedtestStore = defineStore("speedtest", () => {
  // 延迟测速结果映射 nodeTag -> ms
  const latencyMap = ref<Record<string, number>>({});
  // 吞吐量测速结果映射 nodeTag -> ThroughputResult
  const throughputMap = ref<Record<string, ThroughputResult>>({});

  // 正在独立测速的节点 Tag
  const testingNodes = ref<Set<string>>(new Set());

  // 批量测速状态
  const isBatchTesting = ref(false);
  const batchProgress = ref<BatchProgressPayload | null>(null);

  // 初始化监听
  let unlistenProgress: (() => void) | null = null;

  async function init() {
    const res = await getSpeedTestResults();
    if (res.success && res.data) {
      throughputMap.value = res.data;
    }

    if (!unlistenProgress) {
      unlistenProgress = await listenSpeedTestProgress((payload) => {
        batchProgress.value = payload;
        isBatchTesting.value = payload.current_index < payload.total;
        if (payload.result && payload.current_node) {
          throughputMap.value[payload.current_node] = payload.result;
        }
      });
    }
  }

  /** 单节点延迟测试 */
  async function testLatency(groupTag: string, nodeTags: string[]) {
    const res = await runLatencyTest(groupTag, nodeTags);
    if (res.success && res.data) {
      latencyMap.value = { ...latencyMap.value, ...res.data };
    }
    return res;
  }

  /** 单节点吞吐量测试 */
  async function testSingleThroughput(nodeTag: string) {
    testingNodes.value.add(nodeTag);
    const res = await runSingleThroughputTest(nodeTag);
    testingNodes.value.delete(nodeTag);

    if (res.success && res.data) {
      throughputMap.value = { ...throughputMap.value, [nodeTag]: res.data };
    }
    return res;
  }

  /** 启动批量测速 */
  async function startBatchTest(groupTag: string, nodeTags: string[]) {
    isBatchTesting.value = true;
    batchProgress.value = {
      current_index: 0,
      total: nodeTags.length,
      current_node: "",
    };
    return await runBatchSpeedTest(groupTag, nodeTags);
  }

  /** 取消批量测速 */
  async function cancelBatch() {
    await cancelBatchSpeedTest();
    isBatchTesting.value = false;
    batchProgress.value = null;
  }

  // 节点按延迟/网速排序 helper
  const getLatencyColor = (ms?: number): string => {
    if (!ms || ms <= 0) return "#94a3b8";
    if (ms < 120) return "#4ade80"; // 绿色 优
    if (ms < 280) return "#fbbf24"; // 黄色 中
    return "#f87171"; // 红色 劣
  };

  return {
    latencyMap,
    throughputMap,
    testingNodes,
    isBatchTesting,
    batchProgress,
    init,
    testLatency,
    testSingleThroughput,
    startBatchTest,
    cancelBatch,
    getLatencyColor,
  };
});
