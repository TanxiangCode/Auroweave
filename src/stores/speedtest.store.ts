/**
 * Pinia Store — 测速任务管理
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref } from "vue";
import type { SpeedTestTask } from "@/types";
import {
  runSingleSpeedTest,
  runBatchSpeedTest,
  cancelBatchSpeedTest,
  getSpeedTestResults,
} from "@/api/ipc/speedtest";

export const useSpeedTestStore = defineStore("speedtest", () => {
  // ---- 状态 ----
  /** 各节点的测速任务状态 key=nodeTag */
  const tasks = ref<Map<string, SpeedTestTask>>(new Map());
  const batchRunning = ref(false);

  // ---- 单节点测速 ----
  async function testSingle(nodeTag: string) {
    // 设置进行中状态
    tasks.value.set(nodeTag, {
      node_tag: nodeTag,
      status: "testing_speed",
      progress: 0,
    });

    const res = await runSingleSpeedTest(nodeTag);

    if (res.success && res.data) {
      tasks.value.set(nodeTag, {
        node_tag: nodeTag,
        status: "done",
        result: { ...res.data, latency: -1, tested_at: Date.now() },
      });
    } else {
      tasks.value.set(nodeTag, {
        node_tag: nodeTag,
        status: "error",
        error: res.error ?? "测速失败",
      });
    }
  }

  // ---- 批量测速 ----
  async function testBatch(groupTag: string) {
    batchRunning.value = true;
    await runBatchSpeedTest(groupTag);
    batchRunning.value = false;
    // 完成后刷新所有结果
    await refreshResults();
  }

  async function cancelBatch() {
    await cancelBatchSpeedTest();
    batchRunning.value = false;
  }

  async function refreshResults() {
    const res = await getSpeedTestResults();
    if (res.success && res.data) {
      res.data.forEach((task) => {
        tasks.value.set(task.node_tag, task);
      });
    }
  }

  /** 获取某节点的测速状态（便于组件使用） */
  function getTask(nodeTag: string): SpeedTestTask | undefined {
    return tasks.value.get(nodeTag);
  }

  return {
    tasks,
    batchRunning,
    testSingle,
    testBatch,
    cancelBatch,
    refreshResults,
    getTask,
  };
});
