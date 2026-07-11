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
import {
  THROUGHPUT_TEST_DURATION_SEC,
  THROUGHPUT_TEST_CHUNK_BYTES,
  LATENCY_TEST_TIMEOUT_MS,
} from "@/constants";

export const useSpeedtestStore = defineStore("speedtest", () => {
  // 延迟测速结果映射 nodeTag -> ms
  const latencyMap = ref<Record<string, number>>({});
  // 吞吐量测速结果映射 nodeTag -> ThroughputResult
  const throughputMap = ref<Record<string, ThroughputResult>>({});

  // 正在独立测速的节点 Tag
  const testingNodes = ref<Set<string>>(new Set());
  // 正在独立测试延迟的节点 Tag
  const testingLatencyNodes = ref<Set<string>>(new Set());

  // 批量测速状态
  const isBatchTesting = ref(false);
  const batchProgress = ref<BatchProgressPayload | null>(null);

  // 初始化监听
  let unlistenProgress: (() => void) | null = null;

  async function init() {
    const res = await getSpeedTestResults();
    if (res.success && res.data) {
      throughputMap.value = { ...res.data };
    }

    if (!unlistenProgress) {
      unlistenProgress = await listenSpeedTestProgress((payload) => {
        batchProgress.value = payload;
        isBatchTesting.value = payload.current_index < payload.total;
        if (payload.result && payload.current_node) {
          throughputMap.value = {
            ...throughputMap.value,
            [payload.current_node]: payload.result,
          };
        }
      });
    }
  }

  /** 单节点延迟测试 */
  async function testLatency(groupTag: string, nodeTags: string[]) {
    nodeTags.forEach((tag) => testingLatencyNodes.value.add(tag));
    testingLatencyNodes.value = new Set(testingLatencyNodes.value);

    try {
      const res = await runLatencyTest(groupTag, nodeTags);
      if (res.success && res.data) {
        latencyMap.value = { ...latencyMap.value, ...res.data };
      }
      return res;
    } finally {
      nodeTags.forEach((tag) => testingLatencyNodes.value.delete(tag));
      testingLatencyNodes.value = new Set(testingLatencyNodes.value);
    }
  }

  /** 单节点吞吐量测试 */
  async function testSingleThroughput(nodeTag: string) {
    testingNodes.value.add(nodeTag);
    testingNodes.value = new Set(testingNodes.value); // 触发 Vue Set 响应式更新
    
    try {
      const res = await runSingleThroughputTest(nodeTag);
      if (res.success && res.data) {
        throughputMap.value = {
          ...throughputMap.value,
          [nodeTag]: res.data,
        };
      }
      return res;
    } finally {
      testingNodes.value.delete(nodeTag);
      testingNodes.value = new Set(testingNodes.value); // 触发 Vue Set 响应式更新
    }
  }

  /** 开始批量吞吐量测速 */
  async function startBatchTest(groupTag: string, nodeTags: string[]) {
    isBatchTesting.value = true;
    batchProgress.value = null;
    const res = await runBatchSpeedTest(groupTag, nodeTags);
    if (!res.success) {
      isBatchTesting.value = false;
    }
    return res;
  }

  /** 取消批量测速 */
  async function cancelBatch() {
    const res = await cancelBatchSpeedTest();
    isBatchTesting.value = false;
    batchProgress.value = null;
    return res;
  }

  return {
    latencyMap,
    throughputMap,
    testingNodes,
    testingLatencyNodes,
    isBatchTesting,
    batchProgress,
    init,
    testLatency,
    testSingleThroughput,
    startBatchTest,
    cancelBatch,
    THROUGHPUT_TEST_DURATION_SEC,
    THROUGHPUT_TEST_CHUNK_BYTES,
    LATENCY_TEST_TIMEOUT_MS,
  };
});
