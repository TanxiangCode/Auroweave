<template>
  <div class="app-matrix-container">
    <!-- 进程分流生效条件提示：系统代理模式下进程规则静默不匹配 -->
    <div v-if="!tunEnabled" class="mode-hint-banner">
      <BaseIcon name="AlertTriangle" :size="14" />
      <span>当前为系统代理模式：进程分流规则仅在 <strong>TUN 接管模式</strong> 下生效（系统代理下流量源进程是 sing-box 自身，无法按应用区分）。域名/IP 自定义规则不受影响。</span>
    </div>

    <!-- 头部工具栏 -->
    <div class="matrix-toolbar">
      <!-- 搜索框 -->
      <div class="search-box">
        <BaseIcon name="Search" :size="14" class="search-icon" />
        <input
          v-model="searchQuery"
          type="text"
          placeholder="搜索应用名称、进程名或 PID..."
        />
        <button v-if="searchQuery" class="btn-clear" @click="searchQuery = ''"><BaseIcon name="X" :size="12" /></button>
      </div>

      <!-- 分类胶囊过滤 -->
      <div class="category-capsules">
        <button
          class="capsule"
          :class="{ active: selectedCategory === 'all' && !filterCustomOnly }"
          @click="selectCategory('all')"
        >
          <BaseIcon name="Layers" :size="13" /> 全量应用 ({{ filteredProcesses.length }})
        </button>

        <button
          class="capsule active-rules"
          :class="{ active: filterCustomOnly }"
          @click="toggleCustomFilter"
        >
           <BaseIcon name="Star" :size="13" /> 独立绑定 ({{ customCount }})
        </button>

        <button
          class="capsule"
          :class="{ active: selectedCategory === 'developer' }"
          @click="selectCategory('developer')"
        >
           <BaseIcon name="Cpu" :size="13" /> 开发者
        </button>

        <button
          class="capsule"
          :class="{ active: selectedCategory === 'browser' }"
          @click="selectCategory('browser')"
        >
           <BaseIcon name="Globe" :size="13" /> 浏览器
        </button>

        <button
          class="capsule"
          :class="{ active: selectedCategory === 'social' }"
          @click="selectCategory('social')"
        >
           <BaseIcon name="MessageSquare" :size="13" /> 社交
        </button>

        <button
          class="capsule"
          :class="{ active: selectedCategory === 'media' }"
          @click="selectCategory('media')"
        >
           <BaseIcon name="Film" :size="13" /> 影音
        </button>
      </div>

      <!-- 排除系统底层应用开关 -->
      <label class="hide-system-toggle" title="勾选后将自动过滤排除 launchd、WindowServer 等海量系统后台守护进程，聚焦用户前台应用">
        <input type="checkbox" v-model="hideSystemProcesses" />
        <span>排除系统底层</span>
      </label>

      <button class="btn-refresh" @click="fetchData" :disabled="loading">
        <span :class="{ spinning: loading }"></span>
        <span>{{ loading ? '扫描中...' : '刷新进程' }}</span>
      </button>
    </div>

    <!-- 进程列表区 -->
    <div class="matrix-body">
      <div v-if="loading && processes.length === 0" class="state-box glass-effect">
        <span class="spinner"></span>
        <p>正在深度检测活跃系统应用与进程网络栈...</p>
      </div>

      <div v-else-if="filteredProcesses.length === 0" class="state-box glass-effect">
        <span class="empty-icon"></span>
        <p>未找到符合过滤条件的应用进程</p>
      </div>

      <div v-else class="process-grid">
        <div
          v-for="item in filteredProcesses"
          :key="item.proc.pid"
          class="process-card glass-effect"
          :class="{ 'has-custom-rule': Boolean(appRules[item.proc.name]) }"
        >
          <!-- 左侧：应用图标与名称信息 -->
          <div class="card-left">
            <div class="app-icon-wrap" :title="item.appInfo.displayName">
              <img
                v-if="item.proc.icon_base64"
                :src="item.proc.icon_base64"
                class="app-native-icon"
                alt="app icon"
              />
              <BaseIcon
                v-else
                :name="item.appInfo.icon || 'Cpu'"
                :size="20"
                class="app-matrix-icon"
              />
            </div>


            <div class="app-details">
              <div class="name-row">
                <span class="display-name">{{ item.appInfo.displayName }}</span>
                <span class="raw-name" v-if="item.appInfo.displayName !== item.proc.name">
                  ({{ item.proc.name }})
                </span>
                <span class="pid-badge">PID {{ item.proc.pid }}</span>
              </div>
              <div class="path-row" :title="item.proc.exe_path">
                {{ item.proc.exe_path || '系统常驻后台进程' }}
              </div>
            </div>
          </div>

          <!-- 中间：分流指引连线 -->
          <div class="routing-arrow">
            <span class="arrow-line"></span>
            <BaseIcon name="ArrowRight" :size="13" class="arrow-icon" />
          </div>

          <!-- 右侧：出站绑定器 -->
          <div class="card-right">
            <OutboundSelector
              :model-value="appRules[item.proc.name]"
              @change="val => handleRuleChange(item.proc.name, val)"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, computed, onMounted } from "vue";
import {
  getSystemProcesses,
  getAppRules,
  saveAppRule,
  type SystemProcessItem,
} from "@/api/ipc/routing";
import { parseProcessInfo, type ParsedAppInfo } from "@/utils/process-helper";
import { useToast } from "@/composables/useToast";
import { useSettingsStore } from "@/stores/settings.store";
import OutboundSelector from "./OutboundSelector.vue";

interface EnhancedProcess {
  proc: SystemProcessItem;
  appInfo: ParsedAppInfo;
}

const processes = ref<SystemProcessItem[]>([]);
const appRules = ref<Record<string, string>>({});
const searchQuery = ref("");
const filterCustomOnly = ref(false);
const selectedCategory = ref<string>("all");
// 排除系统底层应用选项（默认开启，提升体验）
const hideSystemProcesses = ref(true);
const loading = ref(false);
const toast = useToast();
const settingsStore = useSettingsStore();

// 进程分流仅在 TUN 模式下可匹配（系统代理下流量源进程是 sing-box 自身）
const tunEnabled = computed(() => settingsStore.settings.tun_enabled);

const customCount = computed(() => Object.keys(appRules.value).length);

const enhancedList = computed<EnhancedProcess[]>(() => {
  return processes.value.map((proc) => ({
    proc,
    appInfo: parseProcessInfo(proc.name, proc.exe_path),
  }));
});

function isSystemDaemon(item: EnhancedProcess): boolean {
  // 若用户已经为该进程绑定了规则，始终保留展示
  if (Boolean(appRules.value[item.proc.name])) {
    return false;
  }

  // 类别为系统核心
  if (item.appInfo.category === "system") {
    return true;
  }

  const p = item.proc.exe_path.toLowerCase();
  if (
    p.startsWith("/system/") ||
    p.startsWith("/usr/libexec/") ||
    p.startsWith("/usr/sbin/") ||
    p.includes("c:\windows\system32") ||
    p.includes("c:\windows\syswow64")
  ) {
    return true;
  }

  const name = item.proc.name.toLowerCase();
  if (
    name.endsWith("d") &&
    !item.appInfo.isKnownApp &&
    (name.includes("daemon") || name.includes("service") || name.includes("helper") || name.includes("worker"))
  ) {
    return true;
  }

  return false;
}

const filteredProcesses = computed(() => {
  let list = enhancedList.value;

  // 1. 排除系统底层进程
  if (hideSystemProcesses.value) {
    list = list.filter((item) => !isSystemDaemon(item));
  }

  // 2. 仅看已配置规则
  if (filterCustomOnly.value) {
    list = list.filter((item) => Boolean(appRules.value[item.proc.name]));
  }

  // 3. 分类过滤
  if (selectedCategory.value !== "all") {
    list = list.filter((item) => item.appInfo.category === selectedCategory.value);
  }

  // 4. 关键词搜索
  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((item) => {
      return (
        item.appInfo.displayName.toLowerCase().includes(q) ||
        item.proc.name.toLowerCase().includes(q) ||
        item.proc.pid.toString().includes(q) ||
        item.proc.exe_path.toLowerCase().includes(q)
      );
    });
  }

  return list;
});

function selectCategory(cat: string) {
  filterCustomOnly.value = false;
  selectedCategory.value = cat;
}

function toggleCustomFilter() {
  filterCustomOnly.value = !filterCustomOnly.value;
  if (filterCustomOnly.value) {
    selectedCategory.value = "all";
  }
}

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
  toast.success("应用分流已更新", `[${processName}] → ${outboundTag || '默认策略'}`);
}

onMounted(() => {
  fetchData();
});
</script>

<style scoped>
.app-matrix-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow: hidden;
}

.matrix-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  flex-shrink: 0;
}

/* 系统代理模式下的进程分流提示横幅 */
.mode-hint-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  font-size: 12px;
  line-height: 1.5;
  /* 警示条走 --accent-orange / --status-warning 令牌，
     硬编码 #f5a623 在浅色主题下与浅橙底几乎同色（对比度 1.33），完全读不出来 */
  color: var(--accent-orange);
  background: color-mix(in srgb, var(--accent-orange) 8%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-orange) 25%, transparent);
  border-radius: 10px;
  flex-shrink: 0;
}

.mode-hint-banner strong {
  color: var(--status-warning);
}

.search-box {
  flex: 1;
  min-width: 200px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: 8px;
  transition: all 0.2s ease;
}

.search-box:focus-within {
  background: var(--surface-raised);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
}

.search-icon {
  font-size: 12px;
  opacity: 0.6;
}

.search-box input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 12px;
}

.search-box input::placeholder {
  color: var(--text-tertiary);
}

.btn-clear {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  cursor: pointer;
  font-size: 10px;
}

.category-capsules {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.capsule {
  padding: 4px 10px;
  border-radius: 14px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.capsule:hover {
  background: var(--surface-hover);
  color: var(--text-primary);
}

.capsule.active {
  background: var(--surface-hover);
  border-color: var(--text-tertiary);
  color: var(--text-primary);
}

.capsule.active-rules.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 16%, transparent);
  border-color: var(--accent-cyan-vivid);
  color: var(--accent-cyan-vivid);
}

.hide-system-toggle {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
  padding: 4px 9px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: 7px;
}

.hide-system-toggle input {
  accent-color: var(--accent-cyan-vivid);
  cursor: pointer;
  margin: 0;
}

.btn-refresh {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 10px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-refresh:hover:not(:disabled) {
  background: var(--surface-hover);
  color: var(--text-primary);
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.matrix-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.process-grid {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 3px;
}

.process-card {
  flex-shrink: 0; /* 防止 flex 容器压缩行高（进程较多时被压成横线） */
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  background: var(--surface-inset);
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  gap: 14px;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  flex-shrink: 0;
}

.process-card:hover {
  background: var(--surface-hover);
  border-color: var(--border-strong);
  transform: translateY(-1px);
}

.process-card.has-custom-rule {
  border-left: 3px solid var(--accent-cyan-vivid);
  background: color-mix(in srgb, var(--accent-cyan-vivid) 2.5%, transparent);
}

.card-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.app-icon-wrap {
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  flex-shrink: 0;
  overflow: hidden;
}

.app-native-icon {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  object-fit: contain;
  filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.25));
}


.app-details {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.name-row {
  display: flex;
  align-items: baseline;
  gap: 6px;
}

.display-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.raw-name {
  font-size: 11px;
  color: var(--text-tertiary);
  font-family: monospace;
}

.pid-badge {
  font-size: 9.5px;
  padding: 1px 5px;
  background: var(--surface-raised);
  border-radius: 4px;
  color: var(--text-tertiary);
  font-family: monospace;
}

.path-row {
  font-size: 11px;
  color: var(--text-tertiary);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 420px;
}

.routing-arrow {
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--text-tertiary);
  font-size: 11px;
  flex-shrink: 0;
}

.arrow-line {
  width: 24px;
  height: 1px;
  background: var(--surface-hover);
}

.card-right {
  flex-shrink: 0;
}

.state-box {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-tertiary);
  font-size: 13px;
  border-radius: 12px;
  border: 1px dashed var(--border-normal);
}

.spinner {
  font-size: 24px;
}

.empty-icon {
  font-size: 28px;
  opacity: 0.4;
}
</style>
