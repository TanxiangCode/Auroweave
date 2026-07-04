<template>
  <div class="app-matrix-container">
    <!-- 头部工具栏 -->
    <div class="toolbar">
      <div class="search-box">
        <span class="icon">🔍</span>
        <input
          v-model="searchQuery"
          type="text"
          placeholder="搜索应用名称或进程 ID..."
        />
      </div>

      <div class="filter-capsules">
        <button
          class="capsule"
          :class="{ active: filterCustomOnly }"
          @click="filterCustomOnly = !filterCustomOnly"
        >
          {{ filterCustomOnly ? '已过滤自定义规则 (' + customCount + ')' : '显示全量应用 (' + processes.length + ')' }}
        </button>
        <button class="btn-refresh" @click="fetchData">🔄 刷新进程</button>
      </div>
    </div>

    <!-- 进程应用列表 -->
    <div v-if="loading" class="state-loading">
      ⏳ 正在检测系统进程...
    </div>

    <div v-else-if="filteredProcesses.length === 0" class="state-empty">
      未找到匹配的应用进程
    </div>

    <div v-else class="process-list">
      <div
        v-for="proc in filteredProcesses"
        :key="proc.pid"
        class="process-item glass-effect"
      >
        <div class="proc-icon">📱</div>
        <div class="proc-info">
          <div class="proc-name">
            {{ proc.name }}
            <span class="pid-tag">PID: {{ proc.pid }}</span>
          </div>
          <div class="proc-path" :title="proc.exe_path">
            {{ proc.exe_path || '未知可执行文件路径' }}
          </div>
        </div>

        <div class="proc-outbound">
          <OutboundSelector
            :model-value="appRules[proc.name]"
            @change="val => handleRuleChange(proc.name, val)"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import {
  getSystemProcesses,
  getAppRules,
  saveAppRule,
  type SystemProcessItem,
} from "@/api/ipc/routing";
import { useToast } from "@/composables/useToast";
import OutboundSelector from "./OutboundSelector.vue";

const processes = ref<SystemProcessItem[]>([]);
const appRules = ref<Record<string, string>>({});
const searchQuery = ref("");
const filterCustomOnly = ref(false);
const loading = ref(false);
const toast = useToast();

const customCount = computed(() => Object.keys(appRules.value).length);

const filteredProcesses = computed(() => {
  let list = processes.value;
  if (filterCustomOnly.value) {
    list = list.filter((p) => Boolean(appRules.value[p.name]));
  }
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.pid.toString().includes(q) ||
        p.exe_path.toLowerCase().includes(q)
    );
  }
  return list;
});

async function fetchData() {
  loading.value = true;
  const [procRes, ruleRes] = await Promise.all([
    getSystemProcesses(),
    getAppRules(),
  ]);

  if (procRes.success && procRes.data) {
    processes.value = procRes.data;
  }
  if (ruleRes.success && ruleRes.data) {
    appRules.value = ruleRes.data;
  }
  loading.value = false;
}

async function handleRuleChange(processName: string, outboundTag: string) {
  if (!outboundTag || outboundTag === "default") {
    delete appRules.value[processName];
  } else {
    appRules.value[processName] = outboundTag;
  }
  await saveAppRule(processName, outboundTag);
  toast.success("应用路由分流更新", `[${processName}] ➔ ${outboundTag || '默认'}`);
}

onMounted(() => {
  fetchData();
});
</script>

<style scoped>
.app-matrix-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
}

.search-box {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
}

.search-box input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #fff;
  font-size: 13px;
}

.filter-capsules {
  display: flex;
  gap: 10px;
}

.capsule {
  padding: 6px 12px;
  border-radius: 20px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
  cursor: pointer;
}

.capsule.active {
  background: rgba(0, 242, 254, 0.15);
  border-color: #00f2fe;
  color: #fff;
}

.btn-refresh {
  padding: 6px 12px;
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
  cursor: pointer;
}

.state-loading, .state-empty {
  padding: 40px;
  text-align: center;
  color: rgba(255, 255, 255, 0.4);
  font-size: 13px;
}

.process-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow-y: auto;
}

.process-item {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
}

.proc-icon {
  font-size: 22px;
}

.proc-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.proc-name {
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
  display: flex;
  align-items: center;
  gap: 8px;
}

.pid-tag {
  font-size: 10px;
  padding: 2px 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.4);
}

.proc-path {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  max-width: 400px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
