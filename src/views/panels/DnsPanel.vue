<template>
  <div class="panel-container">
    <h2><BaseIcon name="Globe" :size="20" class="panel-header-icon" /> DNS 安全与延迟测试配置</h2>
    <div class="setting-group">
      <!-- 远端加密 DNS -->
      <div class="setting-item">
        <div class="item-label">
          <span>远端加密 DNS (Remote DoH)</span>
          <span class="sub-label">代理侧加密解析，防 DNS 污染（如 8.8.8.8 / 1.1.1.1），保存后重启内核生效</span>
        </div>
        <input
          v-model="localRemoteDoh"
          type="text"
          class="text-input editable"
          placeholder="8.8.8.8"
          @blur="saveRemoteDoh"
          @keyup.enter="saveRemoteDoh"
        />
      </div>

      <!-- DNS 查询超时 -->
      <div class="setting-item">
        <div class="item-label">
          <span>DNS 查询超时 (Timeout)</span>
          <span class="sub-label">sing-box 1.14 新增：上游无响应快速失败，不再拖默认 10 秒（推荐 3~10 秒）</span>
        </div>
        <div class="input-with-unit">
          <input
            v-model.number="localDnsTimeout"
            type="number"
            min="1"
            max="60"
            class="text-input number-input editable"
            @blur="saveDnsTimeout"
            @keyup.enter="saveDnsTimeout"
          />
          <span class="unit-label">s</span>
        </div>
      </div>

      <!-- 乐观 DNS 缓存 -->
      <div class="setting-item">
        <div class="item-label">
          <span>乐观 DNS 缓存 (Optimistic Cache)</span>
          <span class="sub-label">sing-box 1.14 新增：过期缓存立即返回 + 后台刷新，重复查询零等待；配合 DNS 持久化冷启动秒解析</span>
        </div>
        <input
          type="checkbox"
          v-model="localOptimistic"
          class="switch"
          @change="toggleOptimistic"
        />
      </div>

      <!-- 本地直连 DNS -->
      <div class="setting-item">
        <div class="item-label">
          <span>本地直连 DNS (Local)</span>
          <span class="sub-label">type: local —— 走 macOS 系统原生解析器，自动跟随系统 DNS 配置</span>
        </div>
        <input type="text" value="system (type: local)" class="text-input readonly" readonly tabindex="-1" />
      </div>

      <!-- 延迟测试目标 URL -->
      <div class="setting-item">
        <div class="item-label">
          <span>出站 urltest 延迟测试地址</span>
          <span class="sub-label">Sing-box 健康检测与测延迟的目标 URL</span>
        </div>
        <input
          v-model="localTestUrl"
          type="text"
          class="text-input editable"
          placeholder="http://www.gstatic.com/generate_204"
          @blur="saveLatencyUrl"
          @keyup.enter="saveLatencyUrl"
        />
      </div>

      <!-- 延迟测试并发数量 -->
      <div class="setting-item">
        <div class="item-label">
          <span>延迟测试并发数量 (Concurrency)</span>
          <span class="sub-label">同时发起的测速请求数上限 (默认 20，推荐 10~50 防套接字耗尽)</span>
        </div>
        <div class="input-with-unit">
          <input
            v-model.number="localConcurrency"
            type="number"
            min="1"
            max="100"
            class="text-input number-input editable"
            @blur="saveConcurrency"
            @keyup.enter="saveConcurrency"
          />
          <span class="unit-label">个</span>
        </div>
      </div>

      <!-- 延迟测试超时时间 -->
      <div class="setting-item">
        <div class="item-label">
          <span>延迟测试超时时间 (Timeout)</span>
          <span class="sub-label">单节点请求最大等待时间 (默认 3000ms，推荐 1000~10000ms)</span>
        </div>
        <div class="input-with-unit">
          <input
            v-model.number="localTimeoutMs"
            type="number"
            min="500"
            max="30000"
            step="500"
            class="text-input number-input editable"
            @blur="saveTimeoutMs"
            @keyup.enter="saveTimeoutMs"
          />
          <span class="unit-label">ms</span>
        </div>
      </div>

      <!-- 自动心跳检测间隔 -->
      <div class="setting-item">
        <div class="item-label">
          <span>自动优选探测周期 (Interval) <span class="readonly-tag">只读</span></span>
          <span class="sub-label">内核后台周期性探测间隔 (默认: {{ DEFAULT_LATENCY_TEST_INTERVAL_SEC }} 秒 / 容差 {{ DEFAULT_LATENCY_TEST_TOLERANCE_MS }}ms)</span>
        </div>
        <input type="text" :value="`${DEFAULT_LATENCY_TEST_INTERVAL_SEC}s`" class="text-input readonly" readonly tabindex="-1" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, watch } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import {
  DEFAULT_LATENCY_TEST_URL,
  DEFAULT_LATENCY_TEST_INTERVAL_SEC,
  DEFAULT_LATENCY_TEST_TOLERANCE_MS,
} from "@/constants";

const settingsStore = useSettingsStore();
const toast = useToast();

const localTestUrl = ref(settingsStore.settings.latency_test_url || DEFAULT_LATENCY_TEST_URL);
const localConcurrency = ref(settingsStore.settings.latency_test_concurrency || 20);
const localTimeoutMs = ref(settingsStore.settings.latency_test_timeout_ms || 3000);

// DNS 配置（sing-box 1.14.0）
const localRemoteDoh = ref(settingsStore.settings.dns_remote_doh || "8.8.8.8");
const localDnsTimeout = ref(settingsStore.settings.dns_timeout_secs || 5);
const localOptimistic = ref(settingsStore.settings.dns_optimistic_cache !== false);

// 精确监听相关字段（数组形式 getter），避免深度 watch 整个 settings
// 在无关字段变化时触发无意义的重置
watch(
  () => [
    settingsStore.settings.latency_test_url,
    settingsStore.settings.latency_test_concurrency,
    settingsStore.settings.latency_test_timeout_ms,
    settingsStore.settings.dns_remote_doh,
    settingsStore.settings.dns_timeout_secs,
    settingsStore.settings.dns_optimistic_cache,
  ] as const,
  ([url, concurrency, timeoutMs, doh, dnsTimeout, optimistic]) => {
    localTestUrl.value = url || DEFAULT_LATENCY_TEST_URL;
    localConcurrency.value = concurrency || 20;
    localTimeoutMs.value = timeoutMs || 3000;
    localRemoteDoh.value = doh || "8.8.8.8";
    localDnsTimeout.value = dnsTimeout || 5;
    localOptimistic.value = optimistic !== false;
  }
);

async function saveRemoteDoh() {
  const val = localRemoteDoh.value.trim() || "8.8.8.8";
  localRemoteDoh.value = val;
  const res = await settingsStore.updateSettings({ dns_remote_doh: val });
  if (res.success) {
    toast.success("远端 DoH 已更新", "下次重启内核后生效");
  }
}

async function saveDnsTimeout() {
  const val = Math.max(1, Math.min(60, Math.floor(localDnsTimeout.value || 5)));
  localDnsTimeout.value = val;
  const res = await settingsStore.updateSettings({ dns_timeout_secs: val });
  if (res.success) {
    toast.success("DNS 查询超时已更新", `当前超时: ${val}s`);
  }
}

async function toggleOptimistic() {
  // v-model 已翻转 localOptimistic，这里只负责持久化
  const val = localOptimistic.value;
  const res = await settingsStore.updateSettings({ dns_optimistic_cache: val });
  if (res.success) {
    toast.success("乐观 DNS 缓存已" + (val ? "开启" : "关闭"), "下次重启内核后生效");
  }
}

async function saveLatencyUrl() {
  const url = localTestUrl.value.trim() || DEFAULT_LATENCY_TEST_URL;
  localTestUrl.value = url;
  const res = await settingsStore.updateSettings({ latency_test_url: url });
  if (res.success) {
    toast.success("延迟测试 URL 已更新");
  }
}

async function saveConcurrency() {
  let val = Math.max(1, Math.min(100, Math.floor(localConcurrency.value || 20)));
  localConcurrency.value = val;
  const res = await settingsStore.updateSettings({ latency_test_concurrency: val });
  if (res.success) {
    toast.success("并发数已更新", `当前测试并发上限: ${val}`);
  }
}

async function saveTimeoutMs() {
  let val = Math.max(500, Math.min(30000, Math.floor(localTimeoutMs.value || 3000)));
  localTimeoutMs.value = val;
  const res = await settingsStore.updateSettings({ latency_test_timeout_ms: val });
  if (res.success) {
    toast.success("测试超时已更新", `当前超时阈值: ${val}ms`);
  }
}
</script>

<style scoped>
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

h2 {
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
}

.setting-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 16px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.item-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.sub-label {
  font-size: 11px;
  color: var(--text-tertiary);
  font-weight: normal;
}

.text-input {
  padding: 6px 12px;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: 12px;
  width: 240px;
  outline: none;
  font-family: var(--font-mono);
}

.text-input.editable {
  color: var(--text-primary);
  border-color: var(--border-normal);
  background: var(--layer-3);
  transition: all var(--duration-fast);
}

.text-input.editable:focus {
  border-color: var(--accent-cyan);
  box-shadow: 0 0 6px var(--accent-cyan-glow);
}

/* 只读输入框：明显弱化的不可编辑样式，让用户明确知道该值当前不可配置 */
.text-input.readonly {
  opacity: 0.65;
  cursor: not-allowed;
  border-style: dashed;
  filter: grayscale(0.3);
}

/* "只读"标注徽标 */
.readonly-tag {
  font-size: 10px;
  font-weight: var(--weight-semibold);
  color: var(--text-tertiary);
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  padding: 0 4px;
  margin-left: 4px;
  vertical-align: 1px;
}

.input-with-unit {
  display: flex;
  align-items: center;
  gap: 8px;
}

.number-input {
  width: 100px;
  text-align: right;
}

.unit-label {
  font-size: 12px;
  color: var(--text-tertiary);
  min-width: 20px;
}
</style>

