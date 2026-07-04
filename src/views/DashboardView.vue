<script setup lang="ts">
/**
 * Dashboard 首页 — 中央能量核 + 三张启动卡片
 * 作者: TanXiang
 *
 * TODO(模块K): 接入 ControlCapsule、能量核动画（useFluidWave）
 * TODO(模块B): 接入真实连接状态数据
 */
import { useConnectionStore } from "@/stores/connection.store";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";

const router = useRouter();
const connectionStore = useConnectionStore();
const { smoothDownloadSpeed, activeConnectionCount } = storeToRefs(connectionStore);

/** 格式化速度显示 */
function formatSpeed(bps: number): string {
  if (bps < 1024) return `${bps.toFixed(0)} B/s`;
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(1)} KB/s`;
  return `${(bps / (1024 * 1024)).toFixed(2)} MB/s`;
}

const cards = [
  { id: "proxies", icon: "🌐", label: "代理节点", desc: "选择出站节点", route: "/proxies" },
  { id: "routing", icon: "🛠️", label: "分流配置", desc: "应用级流量规则", route: "/routing" },
  { id: "audit",   icon: "🔍", label: "安全审计", desc: "DNS 解析与连接监控", route: "/audit" },
];
</script>

<template>
  <div class="dashboard">
    <!-- 中央能量核（TODO 模块K：替换为真实动画组件） -->
    <div class="energy-core" :class="{ connected: activeConnectionCount > 0 }">
      <div class="energy-ring">
        <div class="energy-inner">
          <span class="energy-status">
            {{ activeConnectionCount > 0 ? "🟢 运行中" : "⚫ 未连接" }}
          </span>
          <span class="energy-speed">
            ⚡ {{ formatSpeed(smoothDownloadSpeed) }}
          </span>
        </div>
      </div>
    </div>

    <!-- 三张启动卡片 -->
    <div class="launch-cards">
      <button
        v-for="card in cards"
        :key="card.id"
        class="launch-card"
        @click="router.push(card.route)"
      >
        <span class="card-icon">{{ card.icon }}</span>
        <span class="card-label">{{ card.label }}</span>
        <span class="card-desc">{{ card.desc }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--space-10);
  padding: var(--space-8);
}

/* ---- 能量核 ---- */
.energy-core {
  display: flex;
  align-items: center;
  justify-content: center;
}

.energy-ring {
  width: 180px;
  height: 180px;
  border-radius: 50%;
  padding: 4px;
  background: conic-gradient(
    from 0deg,
    var(--accent-blue),
    var(--accent-cyan),
    var(--accent-green),
    var(--accent-blue)
  );
  animation: ring-spin 4s linear infinite;
  box-shadow: var(--shadow-glow-cyan);
}

.energy-core:not(.connected) .energy-ring {
  background: conic-gradient(from 0deg, #2d2d2d, #3f3f46, #2d2d2d);
  box-shadow: none;
  animation: ring-breathe 3s ease-in-out infinite;
}

.energy-inner {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: var(--layer-0);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
}

.energy-status {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-weight: var(--weight-medium);
}

.energy-speed {
  font-size: var(--text-md);
  color: var(--text-primary);
  font-weight: var(--weight-semibold);
}

/* ---- 启动卡片 ---- */
.launch-cards {
  display: flex;
  gap: var(--space-4);
}

.launch-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-5) var(--space-6);
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  color: var(--text-primary);
  cursor: pointer;
  min-width: 140px;
  transition:
    background var(--duration-fast) var(--ease-default),
    border-color var(--duration-fast) var(--ease-default),
    transform var(--duration-fast) var(--ease-default);
}

.launch-card:hover {
  background: var(--layer-2);
  border-color: var(--border-strong);
}

.launch-card:active {
  transform: scale(0.98);
}

.card-icon {
  font-size: 28px;
  line-height: 1;
}

.card-label {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
}

.card-desc {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  text-align: center;
}

/* ---- 动画 ---- */
@keyframes ring-spin {
  to { transform: rotate(360deg); }
}

@keyframes ring-breathe {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 0.7; }
}

/* 性能模式降级 */
[data-perf-mode="reduced"] .energy-ring {
  animation: none;
}
</style>
