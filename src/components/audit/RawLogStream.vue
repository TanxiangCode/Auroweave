<template>
  <div class="raw-log-container">
    <div class="log-toolbar">
      <span class="log-title">🧑‍💻 Sing-box 内核原始日志流</span>
      <div class="actions">
        <label class="auto-scroll-label">
          <input type="checkbox" v-model="autoScroll" />
          自动底端滚动
        </label>
        <button class="btn-clear" @click="logs = []">清空日志</button>
      </div>
    </div>

    <div class="log-content" ref="logContainerRef">
      <div v-if="logs.length === 0" class="empty-tip">
        等待内核日志输出...
      </div>
      <div
        v-for="(line, index) in logs"
        :key="index"
        class="log-line"
        :class="getLogLevelClass(line)"
      >
        {{ line }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from "vue";
import { subscribeLog } from "@/api/clash-ws";

const logs = ref<string[]>([]);
const autoScroll = ref(true);
const logContainerRef = ref<HTMLDivElement | null>(null);

let unsub: (() => void) | null = null;

onMounted(() => {
  unsub = subscribeLog((line: string) => {
    logs.value.push(line);
    if (logs.value.length > 500) {
      logs.value.shift();
    }
    if (autoScroll.value) {
      nextTick(() => {
        if (logContainerRef.value) {
          logContainerRef.value.scrollTop = logContainerRef.value.scrollHeight;
        }
      });
    }
  });
});

onUnmounted(() => {
  if (unsub) unsub();
});

function getLogLevelClass(line: string): string {
  const lower = line.toLowerCase();
  if (lower.includes("error") || lower.includes("fatal")) return "error";
  if (lower.includes("warn")) return "warn";
  if (lower.includes("info")) return "info";
  return "default";
}
</script>

<style scoped>
.raw-log-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: rgba(10, 12, 18, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  overflow: hidden;
}

.log-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 16px;
  background: rgba(255, 255, 255, 0.04);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  font-size: 13px;
}

.log-title {
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
}

.actions {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
}

.auto-scroll-label {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.btn-clear {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.7);
  padding: 2px 8px;
  font-size: 11px;
  cursor: pointer;
}

.log-content {
  flex: 1;
  padding: 12px 16px;
  font-family: monospace;
  font-size: 12px;
  line-height: 1.6;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.empty-tip {
  color: rgba(255, 255, 255, 0.3);
  text-align: center;
  padding: 40px;
}

.log-line {
  word-break: break-all;
}
.log-line.error { color: #f87171; }
.log-line.warn { color: #fbbf24; }
.log-line.info { color: #00f2fe; }
.log-line.default { color: rgba(255, 255, 255, 0.7); }
</style>
