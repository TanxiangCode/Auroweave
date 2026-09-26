<template>
  <div class="connectivity-panel">
    <div class="panel-header">
      <div class="header-left">
        <span class="pulse-dot" :class="{ ok: report && report.kernel_running }"></span>
        <span class="title">连通性与出口检测</span>
        <span class="sub">双路径探测：直连基线 vs 经代理出口，判定流量走向与直连泄漏</span>
      </div>
      <button class="btn-run" :disabled="checking" @click="runCheck">
        <span v-if="checking" class="spinner"></span>
        {{ checking ? "探测中…" : "立即检测" }}
      </button>
    </div>

    <!-- 检测结果 -->
    <div v-if="report" class="result-grid">
      <!-- 直连基线 -->
      <div class="path-card direct">
        <div class="path-head">
          <span class="path-name">直连基线 (不经代理)</span>
          <span class="path-status" :class="report.direct.ok ? 'ok' : 'fail'">
            {{ report.direct.ok ? "可用" : "失败" }}
          </span>
        </div>
        <template v-if="report.direct.ok">
          <div class="ip-line">{{ report.direct.egress_ip }}</div>
          <div class="loc-line">{{ report.direct.location }}</div>
        </template>
        <div v-else class="err-line">{{ report.direct.error || "探测失败" }}</div>
        <div class="ms-line">{{ report.direct.elapsed_ms }}ms</div>
      </div>

      <!-- 代理出口 -->
      <div class="path-card proxied">
        <div class="path-head">
          <span class="path-name">代理出口 (经 mixed 端口)</span>
          <span class="path-status" :class="report.proxied.ok ? 'ok' : 'fail'">
            {{ report.proxied.ok ? "可用" : (report.kernel_running ? "失败" : "内核未运行") }}
          </span>
        </div>
        <template v-if="report.proxied.ok">
          <div class="ip-line">{{ report.proxied.egress_ip }}</div>
          <div class="loc-line">{{ report.proxied.location }}</div>
        </template>
        <div v-else class="err-line">{{ report.proxied.error || (report.kernel_running ? "探测失败" : "启动内核后可检测代理路径") }}</div>
        <div class="ms-line">{{ report.proxied.elapsed_ms }}ms</div>
      </div>
    </div>

    <!-- 判定结论 -->
    <div v-if="report" class="verdict-area">
      <div v-if="report.traffic_proxied" class="verdict ok">
        <span class="v-icon">✓</span>
        <div>
          <p class="v-title">流量确认经代理出口</p>
          <p class="v-desc">代理路径出口 IP（{{ report.proxied.egress_ip }}）与直连出口不同，分流与代理链路工作正常。</p>
        </div>
      </div>
      <div v-else-if="report.leak_suspect && report.kernel_running" class="verdict warn">
        <span class="v-icon">!</span>
        <div>
          <p class="v-title">疑似直连泄漏</p>
          <p class="v-desc">两条路径出口 IP 相同（{{ report.direct.egress_ip }}）——代理路径的流量实际走了直连出口。请检查当前模式是否为 Direct、或节点是否可用。</p>
        </div>
      </div>
      <div v-else-if="!report.kernel_running" class="verdict info">
        <span class="v-icon">i</span>
        <div>
          <p class="v-title">内核未运行</p>
          <p class="v-desc">代理路径探测结果不可用；直连基线独立有效。启动内核后再次检测可完成对比判定。</p>
        </div>
      </div>
      <div v-else-if="!report.direct.ok" class="verdict info">
        <span class="v-icon">i</span>
        <div>
          <p class="v-title">直连基线不可用</p>
          <p class="v-desc">本机直连访问 ip-api.com 失败（网络限制或断网），泄漏判定需要双路径都成功。</p>
        </div>
      </div>
    </div>

    <!-- 空态 -->
    <div v-else class="empty-state">
      <span class="empty-icon"><BaseIcon name="Radio" :size="40" /></span>
      <p>点击「立即检测」发起双路径出口探测</p>
      <p class="empty-sub">检测目标：ip-api.com · 判定依据：双路径出口 IP 对比</p>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 连通性与出口检测面板
 * 作者: TanXiang
 *
 * 双路径探测（后端 proxy_connectivity_check）：
 * - 直连基线：不经任何代理直接访问，取得本机真实出口 IP
 * - 代理出口：经本地 mixed 端口，取得当前代理链路的出口 IP
 * 对比判定：IP 不同=流量经代理 ✓；IP 相同=疑似直连泄漏
 */
import { ref } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { invokeWithTimeout } from "@/api/ipc/client";
import { useToast } from "@/composables/useToast";
import type { ApiResponse } from "@/types";

interface PathResult {
  path: string;
  ok: boolean;
  egress_ip: string;
  location: string;
  /** 国家/地区代码（ISO 3166-1 alpha-2，供国旗渲染） */
  country_code: string;
  elapsed_ms: number;
  error: string;
}

interface ConnectivityReport {
  direct: PathResult;
  proxied: PathResult;
  traffic_proxied: boolean;
  leak_suspect: boolean;
  kernel_running: boolean;
}

const report = ref<ConnectivityReport | null>(null);
const checking = ref(false);
const toast = useToast();

async function runCheck() {
  checking.value = true;
  try {
    const res = await invokeWithTimeout<ApiResponse<ConnectivityReport>>(
      "proxy_connectivity_check",
      {},
      20000
    );
    if (res.success && res.data) {
      report.value = res.data;
    } else {
      toast.error("检测失败", res.error || "探测请求异常");
    }
  } finally {
    checking.value = false;
  }
}
</script>

<style scoped>
.connectivity-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  overflow-y: auto;
  padding: 4px;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-tertiary);
  box-shadow: 0 0 6px var(--text-tertiary);
}

.pulse-dot.ok {
  background: var(--accent-green);
  box-shadow: 0 0 8px var(--accent-green);
  animation: pulse 2s ease infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.title {
  font-size: 15px;
  font-weight: 700;
  color: var(--text-primary);
}

.sub {
  font-size: 11px;
  color: var(--text-tertiary);
}

.btn-run {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 20px;
  font-size: 12px;
  font-weight: 600;
  color: var(--accent-cyan-vivid);
  background: color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-run:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 20%, transparent);
  color: var(--text-primary);
}

.btn-run:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.spinner {
  width: 12px;
  height: 12px;
  border: 2px solid color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  border-top-color: var(--accent-cyan-vivid);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.result-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 12px;
}

.path-card {
  padding: 16px;
  border-radius: 12px;
  border: 1px solid var(--border-subtle);
  background: var(--layer-2);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.path-card.direct { border-left: 3px solid var(--accent-green); }
.path-card.proxied { border-left: 3px solid var(--accent-cyan-vivid); }

.path-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.path-name {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
}

.path-status {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 10px;
}

.path-status.ok {
  color: var(--accent-green);
  background: rgba(16, 185, 129, 0.1);
}

.path-status.fail {
  color: var(--status-danger);
  background: color-mix(in srgb, var(--status-danger) 10%, transparent);
}

.ip-line {
  font-family: var(--font-mono);
  font-size: 18px;
  font-weight: 700;
  color: var(--text-primary);
}

.loc-line {
  font-size: 12px;
  color: var(--text-secondary);
}

.err-line {
  font-size: 11px;
  color: var(--text-tertiary);
}

.ms-line {
  font-size: 11px;
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  margin-top: 4px;
}

.verdict-area {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.verdict {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  padding: 14px 16px;
  border-radius: 12px;
  border: 1px solid var(--border-subtle);
}

.verdict.ok {
  background: rgba(16, 185, 129, 0.06);
  border-color: rgba(16, 185, 129, 0.3);
}

.verdict.warn {
  background: color-mix(in srgb, var(--accent-orange) 6%, transparent);
  border-color: color-mix(in srgb, var(--accent-orange) 30%, transparent);
}

.verdict.info {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 4%, transparent);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 20%, transparent);
}

.v-icon {
  font-size: 16px;
  font-weight: 700;
  line-height: 1;
  margin-top: 2px;
}

.verdict.ok .v-icon { color: var(--accent-green); }
.verdict.warn .v-icon { color: var(--accent-orange); }
.verdict.info .v-icon { color: var(--accent-cyan-vivid); }

.v-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0 0 4px;
}

.v-desc {
  font-size: 12px;
  color: var(--text-secondary);
  line-height: 1.5;
  margin: 0;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: var(--text-tertiary);
  font-size: 13px;
}

.empty-icon {
  font-size: 36px;
  opacity: 0.6;
}

.empty-sub {
  font-size: 11px;
  opacity: 0.6;
}
</style>
