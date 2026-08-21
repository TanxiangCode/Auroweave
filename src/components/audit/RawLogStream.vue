<template>
  <div class="raw-log-container glass-effect">
    <div class="log-toolbar">
      <div class="toolbar-left">
        <span class="log-title">🧑‍💻 Sing-box 内核终端日志流</span>
        <span class="log-count">({{ filteredLogs.length }} / {{ logs.length }} 行)</span>
      </div>

      <div class="toolbar-center">
        <input
          v-model="searchKey"
          type="text"
          class="log-filter-input"
          placeholder="过滤内核日志关键词..."
        />
        <div class="level-pills">
          <button
            class="level-btn"
            :class="{ active: levelFilter === 'all' }"
            @click="levelFilter = 'all'"
          >
            全部
          </button>
          <button
            class="level-btn info"
            :class="{ active: levelFilter === 'info' }"
            @click="levelFilter = 'info'"
          >
            INFO
          </button>
          <button
            class="level-btn warn"
            :class="{ active: levelFilter === 'warn' }"
            @click="levelFilter = 'warn'"
          >
            WARN
          </button>
          <button
            class="level-btn error"
            :class="{ active: levelFilter === 'error' }"
            @click="levelFilter = 'error'"
          >
            ERROR
          </button>
        </div>
      </div>

      <div class="toolbar-right">
        <label class="auto-scroll-label">
          <input type="checkbox" v-model="autoScroll" />
          自动滚底
        </label>
        <button class="btn-tool" @click="copyAllLogs" title="复制当前可见日志">📋 复制</button>
        <button class="btn-tool danger" @click="logs = []">清空日志</button>
      </div>
    </div>

    <div class="log-content" ref="logContainerRef">
      <div v-if="filteredLogs.length === 0" class="empty-tip">
        {{ logs.length === 0 ? '等待内核日志输出中...' : '无匹配过滤条件的日志' }}
      </div>
      <div
        v-for="(line, index) in filteredLogs"
        :key="index"
        class="log-line"
        :class="getLogLevelClass(line)"
      >
        <span class="line-num">{{ index + 1 }}</span>
        <span class="line-text">{{ line }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from "vue";
import { subscribeLog } from "@/api/clash-ws";
import { useToast } from "@/composables/useToast";

const logs = ref<string[]>([]);
const searchKey = ref("");
const levelFilter = ref<"all" | "info" | "warn" | "error">("all");
const autoScroll = ref(true);
const logContainerRef = ref<HTMLDivElement | null>(null);
const toast = useToast();

let unsub: (() => void) | null = null;

const filteredLogs = computed(() => {
  let list = logs.value;
  const q = searchKey.value.trim().toLowerCase();

  if (levelFilter.value !== "all") {
    list = list.filter((line) => {
      const lower = line.toLowerCase();
      if (levelFilter.value === "error") return lower.includes("error") || lower.includes("fatal");
      if (levelFilter.value === "warn") return lower.includes("warn");
      if (levelFilter.value === "info") return lower.includes("info");
      return true;
    });
  }

  if (q) {
    list = list.filter((line) => line.toLowerCase().includes(q));
  }

  return list;
});

onMounted(() => {
  unsub = subscribeLog((line: string) => {
    logs.value.push(line);
    if (logs.value.length > 1000) {
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

function copyAllLogs() {
  const text = filteredLogs.value.join("\n");
  navigator.clipboard.writeText(text);
  toast.success("日志已复制", `共 ${filteredLogs.value.length} 行`);
}
</script>

<style scoped>
.raw-log-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: rgba(14, 16, 22, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 14px;
  overflow: hidden;
}

.log-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 16px;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 12px;
  gap: 12px;
  flex-wrap: wrap;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.log-title {
  font-weight: 600;
  color: #fff;
}

.log-count {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.toolbar-center {
  display: flex;
  align-items: center;
  gap: 8px;
}

.log-filter-input {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 4px 10px;
  color: #fff;
  font-size: 11px;
  outline: none;
  width: 160px;
}

.log-filter-input:focus {
  border-color: rgba(0, 242, 254, 0.4);
}

.level-pills {
  display: flex;
  background: rgba(255, 255, 255, 0.04);
  border-radius: 6px;
  padding: 2px;
}

.level-btn {
  padding: 2px 8px;
  font-size: 10px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
}

.level-btn.active {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  font-weight: 600;
}

.level-btn.info.active { color: #00f2fe; }
.level-btn.warn.active { color: #fbbf24; }
.level-btn.error.active { color: #f87171; }

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.auto-scroll-label {
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  color: rgba(255, 255, 255, 0.6);
  font-size: 11px;
}

.btn-tool {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.8);
  padding: 3px 8px;
  font-size: 11px;
  cursor: pointer;
}

.btn-tool:hover {
  background: rgba(255, 255, 255, 0.1);
  color: #fff;
}

.btn-tool.danger:hover {
  background: rgba(239, 68, 68, 0.2);
  border-color: #ef4444;
  color: #ef4444;
}

.log-content {
  flex: 1;
  padding: 12px 16px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11.5px;
  line-height: 1.6;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.empty-tip {
  color: rgba(255, 255, 255, 0.3);
  text-align: center;
  padding: 40px;
}

.log-line {
  display: flex;
  gap: 12px;
  word-break: break-all;
}

.line-num {
  color: rgba(255, 255, 255, 0.2);
  user-select: none;
  min-width: 32px;
  text-align: right;
  font-size: 10px;
}

.line-text {
  flex: 1;
}

.log-line.error { color: #f87171; }
.log-line.warn { color: #fbbf24; }
.log-line.info { color: #00f2fe; }
.log-line.default { color: rgba(255, 255, 255, 0.7); }
</style>
