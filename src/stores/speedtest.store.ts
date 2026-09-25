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
  cancelLatencyTest,
  getSpeedTestResults,
  listenSpeedTestProgress,
  listenLatencyTestProgress,
  type BatchProgressPayload,
} from "@/api/ipc/speedtest";
import {
  THROUGHPUT_TEST_DURATION_SEC,
  THROUGHPUT_TEST_CHUNK_BYTES,
  LATENCY_TEST_TIMEOUT_MS,
} from "@/constants";
import { useToast } from "@/composables/useToast";

export const useSpeedtestStore = defineStore("speedtest", () => {
  // 延迟测速结果映射 nodeTag -> ms
  const latencyMap = ref<Record<string, number>>({});
  // 吞吐量测速结果映射 nodeTag -> ThroughputResult
  const throughputMap = ref<Record<string, ThroughputResult>>({});

  // 正在独立测速的节点 Tag
  const testingNodes = ref<Set<string>>(new Set());
  // 正在独立测试延迟的节点 Tag
  const testingLatencyNodes = ref<Set<string>>(new Set());
  // 是否正在进行批量/全局延迟测试（并发计数归零时才为 false）
  const isTestingLatency = ref(false);
  // 批量测延迟进度
  const latencyBatchProgress = ref<BatchProgressPayload | null>(null);
  // 用户已请求取消，等待后端终止事件统一收尾
  const latencyBatchCancelled = ref(false);

  // 批量吞吐量测速状态
  const isBatchTesting = ref(false);
  const batchProgress = ref<BatchProgressPayload | null>(null);

  // 用户已请求取消但后端尚未确认（终止事件未到）
  const batchCancelled = ref(false);

  // 并发延迟测试计数：支持多批次同时进行
  let activeLatencyTests = 0;

  // 初始化监听
  let unlistenProgress: (() => void) | null = null;
  let unlistenLatencyProgress: (() => void) | null = null;

  // init() 幂等哨兵：并发调用共享同一 Promise，防止重复注册事件监听
  let initPromise: Promise<void> | null = null;

  /** 从后端重新同步持久化/内存测速结果；当前会话已收到的数据优先保留。 */
  async function syncThroughputResults() {
    try {
      const res = await getSpeedTestResults();
      if (res.success && res.data) {
        throughputMap.value = { ...res.data, ...throughputMap.value };
      }
    } catch {
      // 结果事件已实时写入；同步失败不应影响任务完成状态。
    }
  }

  async function init() {
    if (initPromise) return initPromise;
    initPromise = (async () => {
      const res = await getSpeedTestResults();
      if (res.success && res.data) {
        throughputMap.value = { ...res.data };
      }

      if (!unlistenProgress) {
        unlistenProgress = await listenSpeedTestProgress((payload) => {
          // 终止事件：后端在批次结束（完成或取消）时发送
          // current_index === total 且 current_node 为空串的哨兵事件
          const isTerminal =
            payload.current_index === payload.total &&
            payload.total > 0 &&
            !payload.current_node;

          if (isTerminal) {
            // 终止事件统一复位一切状态（正常完成或取消），并从持久层补齐可能错过的结果。
            isBatchTesting.value = false;
            batchCancelled.value = false;
            batchProgress.value = null;
            void syncThroughputResults();
            return;
          }

          // 取消等待期：忽略普通进度事件，等终止事件统一收尾，
          // 防止后端未及时停止时 UI 卡在"测速中"
          if (batchCancelled.value) return;

          batchProgress.value = payload;
          // 注意：最后一个节点的"开始测速"事件同样满足 current_index === total，
          // 因此不能仅凭 index 判断批次结束，需依赖终止事件
          isBatchTesting.value = payload.current_index <= payload.total;
          if (payload.result && payload.current_node) {
            throughputMap.value = {
              ...throughputMap.value,
              [payload.current_node]: payload.result,
            };
          }
        });
      }

      if (!unlistenLatencyProgress) {
        unlistenLatencyProgress = await listenLatencyTestProgress((payload) => {
          const isTerminal =
            payload.current_index === payload.total &&
            payload.total > 0 &&
            !payload.current_node;

          if (isTerminal) {
            latencyBatchProgress.value = null;
            latencyBatchCancelled.value = false;
            return;
          }

          // 单节点测试或已结束批次也会走同一事件流；没有活动批量进度时只回填结果，
          // 绝不能让迟到事件重新创建底部 Dock。
          if (payload.current_node && payload.delay >= 0 && !latencyBatchCancelled.value) {
            latencyMap.value = {
              ...latencyMap.value,
              [payload.current_node]: payload.delay,
            };
          }

          const hasActiveBatch = latencyBatchProgress.value !== null;
          if (!hasActiveBatch || latencyBatchCancelled.value) return;

          latencyBatchProgress.value = {
            current_index: payload.current_index,
            total: payload.total,
            current_node: payload.current_node,
          };
        });
      }
    })().catch((e) => {
      // 初始化失败不应缓存失败的 Promise，允许下次重试
      initPromise = null;
      throw e;
    });
    return initPromise;
  }

  /** 延迟测试 (支持单个或多个节点，支持多批次并发) */
  async function testLatency(groupTag: string, nodeTags: string[]) {
    // 监听必须先于 invoke 注册，避免页面刚激活便立即点击时漏掉整批结果事件。
    // init 已完成时不额外让出微任务，保持“调用即出现 0/total 进度”的交互契约。
    if (!initPromise) await init();

    // 记录本批次 tags 快照：并发场景下 finally 只删自己批次的节点，
    // 避免批次 A 结束时误删批次 B 仍在测试中的标记
    const batchTags = [...nodeTags];

    activeLatencyTests++;
    isTestingLatency.value = true;
    latencyBatchCancelled.value = false;
    batchTags.forEach((tag) => testingLatencyNodes.value.add(tag));
    testingLatencyNodes.value = new Set(testingLatencyNodes.value);

    // 批量测试（大于1个节点）时，初始化批量进度条
    if (batchTags.length > 1) {
      latencyBatchProgress.value = {
        current_index: 0,
        total: batchTags.length,
        current_node: "正在准备测延迟...",
      };
    }

    try {
      const res = await runLatencyTest(groupTag, nodeTags);
      const newResults = { ...latencyMap.value };

      if (res.success && res.data) {
        // 后端现在返回所有测试过的节点：delay > 0 为成功，delay = 0 为失败
        Object.entries(res.data).forEach(([tag, delay]) => {
          newResults[tag] = delay as number;
        });
      } else {
        // IPC 层面失败（如 sing-box 未运行），所有节点标记为 -1
        batchTags.forEach((tag) => {
          newResults[tag] = -1;
        });
      }

      latencyMap.value = newResults;
      return res;
    } catch (e) {
      // 捕获异常：将本批次测速节点强制设为 -1
      const newResults = { ...latencyMap.value };
      batchTags.forEach((tag) => {
        newResults[tag] = -1;
      });
      latencyMap.value = newResults;
      throw e;
    } finally {
      batchTags.forEach((tag) => testingLatencyNodes.value.delete(tag));
      testingLatencyNodes.value = new Set(testingLatencyNodes.value);
      activeLatencyTests--;
      if (activeLatencyTests <= 0) {
        activeLatencyTests = 0; // 容错：异常路径下防止负数
        isTestingLatency.value = false;
        latencyBatchProgress.value = null;
        latencyBatchCancelled.value = false;
      }
    }
  }

  /** 取消批量延迟测试 */
  async function cancelLatencyBatch() {
    latencyBatchCancelled.value = true;
    const res = await cancelLatencyTest();
    if (!res.success) latencyBatchCancelled.value = false;
    return res;
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
    if (!initPromise) await init();
    isBatchTesting.value = true;
    batchCancelled.value = false;
    // 立即铺一条 0/total 进度：批次首个节点结束前后端不发事件，
    // 留空会让进度条（连带取消按钮）在整批首轮探测期间缺席
    batchProgress.value = {
      current_index: 0,
      total: nodeTags.length,
      current_node: "正在准备测速...",
    };
    try {
      const res = await runBatchSpeedTest(groupTag, nodeTags);
      // Rust 端 run_batch 为 fire-and-forget：invoke 结果仅反映"启动是否成功"。
      // 启动失败（reject 或 success=false）时后端没有批次在跑，直接复位即可；
      // 启动成功后 isBatchTesting 由终止事件复位。
      if (!res.success) {
        isBatchTesting.value = false;
        batchProgress.value = null;
        useToast().error("批量测速启动失败", res.error ?? "请确认 Sing-box 核心是否在运行。");
      }
      return res;
    } catch (e) {
      // invoke 本身抛错（超时/后端异常）→ 批次未启动，复位并提示
      isBatchTesting.value = false;
      batchProgress.value = null;
      useToast().error("批量测速启动失败", "与后端通信异常，请稍后重试。");
      throw e;
    }
  }

  /** 取消批量测速 */
  async function cancelBatch() {
    // 立即标记取消，终止事件到达前忽略进度事件；
    // isBatchTesting 等状态由后端终止事件统一复位
    batchCancelled.value = true;
    const res = await cancelBatchSpeedTest();
    if (!res.success) {
      // 取消指令未送达后端：回滚取消标记，恢复进度接收
      batchCancelled.value = false;
    }
    return res;
  }

  return {
    latencyMap,
    throughputMap,
    testingNodes,
    testingLatencyNodes,
    isTestingLatency,
    latencyBatchProgress,
    latencyBatchCancelled,
    isBatchTesting,
    batchProgress,
    batchCancelled,
    init,
    testLatency,
    cancelLatencyBatch,
    testSingleThroughput,
    startBatchTest,
    cancelBatch,
    THROUGHPUT_TEST_DURATION_SEC,
    THROUGHPUT_TEST_CHUNK_BYTES,
    LATENCY_TEST_TIMEOUT_MS,
  };
});
