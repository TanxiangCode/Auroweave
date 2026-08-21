<template>
  <div v-if="record" class="drawer-overlay" @click.self="$emit('close')">
    <div class="drawer-panel glass-effect">
      <!-- 头部 -->
      <div class="drawer-header">
        <div class="header-main">
          <span class="app-icon">{{ record.appIcon }}</span>
          <div class="header-titles">
            <h3>{{ record.appDisplayName }}</h3>
            <span class="header-sub" :title="record.domain">{{ record.domain }}</span>
          </div>
        </div>

        <div class="header-right">
          <span class="status-badge" :class="isActive ? 'online' : 'closed'">
            {{ isActive ? '🟢 活跃连接' : '⚪ 历史会话' }}
          </span>
          <button class="btn-close" @click="$emit('close')">✕</button>
        </div>
      </div>

      <!-- 抽屉主体 -->
      <div class="drawer-body">
        <!-- 语义解说区 -->
        <div class="semantic-box" :class="record.type">
          <div class="semantic-title-row">
            <span class="box-icon">{{ record.type === 'direct' ? '🎯' : record.type === 'blocked' ? '🚫' : '🚀' }}</span>
            <span class="box-title">{{ record.semanticTitle }}</span>
          </div>
          <p class="box-desc">{{ record.semanticDesc }}</p>
          <div class="box-badges">
            <span v-for="(b, idx) in record.securityBadges" :key="idx" class="badge">
              {{ b }}
            </span>
          </div>
        </div>

        <!-- 转发链路可视化 -->
        <div class="section">
          <div class="section-title">🌐 分流路由拓扑链</div>
          <div class="chain-flow">
            <div class="node-step">
              <span class="step-icon">{{ record.appIcon }}</span>
              <span class="step-name">{{ record.appDisplayName }}</span>
            </div>
            <span class="arrow-right">➔</span>
            <div class="node-step">
              <span class="step-icon">📋</span>
              <span class="step-name">{{ record.ruleMatched }}</span>
            </div>
            <span class="arrow-right">➔</span>
            <div class="node-step highlight">
              <span class="step-icon">🚀</span>
              <span class="step-name">{{ record.outbound }}</span>
            </div>
            <span class="arrow-right">➔</span>
            <div class="node-step">
              <span class="step-icon">🎯</span>
              <span class="step-name">{{ record.domain }}</span>
            </div>
          </div>
        </div>

        <!-- 元数据网格 -->
        <div class="section">
          <div class="section-title">🔍 详细网络元数据</div>
          <div class="metadata-grid">
            <div class="meta-item">
              <span class="meta-label">连接 ID</span>
              <div class="meta-val-row">
                <span class="meta-val mono">{{ record.id }}</span>
                <button class="btn-copy" @click="copyText(record.id)">📋</button>
              </div>
            </div>

            <div class="meta-item">
              <span class="meta-label">目标主机 / IP</span>
              <div class="meta-val-row">
                <span class="meta-val mono">{{ record.domain }}</span>
                <button class="btn-copy" @click="copyText(record.domain)">📋</button>
              </div>
            </div>

            <div class="meta-item" v-if="record.destinationIP">
              <span class="meta-label">目标解析 IP</span>
              <div class="meta-val-row">
                <span class="meta-val mono">{{ record.destinationIP }}:{{ record.port }}</span>
                <button class="btn-copy" @click="copyText(`${record.destinationIP}:${record.port}`)">📋</button>
              </div>
            </div>

            <div class="meta-item">
              <span class="meta-label">网络协议</span>
              <span class="meta-val">{{ (record.network || 'TCP').toUpperCase() }} (Port: {{ record.port }})</span>
            </div>

            <div class="meta-item full-width" v-if="record.processPath">
              <span class="meta-label">可执行文件绝对路径</span>
              <div class="meta-val-row">
                <span class="meta-val mono path-text" :title="record.processPath">{{ record.processPath }}</span>
                <button class="btn-copy" @click="copyText(record.processPath)">📋</button>
              </div>
            </div>

            <div class="meta-item">
              <span class="meta-label">下行数据量</span>
              <span class="meta-val text-cyan">↓ {{ formatBytes(record.download_bytes) }}</span>
            </div>

            <div class="meta-item">
              <span class="meta-label">上行数据量</span>
              <span class="meta-val text-purple">↑ {{ formatBytes(record.upload_bytes) }}</span>
            </div>

            <div class="meta-item">
              <span class="meta-label">会话起始时间</span>
              <span class="meta-val">{{ new Date(record.start).toLocaleString('zh-CN') }}</span>
            </div>

            <div class="meta-item">
              <span class="meta-label">持续时长</span>
              <span class="meta-val">{{ getDuration(record.start) }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 底部快捷动作 -->
      <div class="drawer-footer">
        <button
          v-if="record.process"
          class="btn-footer-action primary"
          @click="handleAddAppMatrix(record.process)"
          title="将该应用添加到 App-Matrix 进程路由规则"
        >
          📱 添加至 App-Matrix 进程分流
        </button>

        <button
          v-if="isActive"
          class="btn-footer-action danger"
          @click="handleClose(record.id)"
        >
          🔌 立即切断该连接
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from "vue-router";
import type { SemanticAuditRecord } from "@/utils/semantic-translator";
import { useToast } from "@/composables/useToast";

defineProps<{
  record: SemanticAuditRecord | null;
  isActive: boolean;
}>();


const emit = defineEmits<{
  (e: "close"): void;
  (e: "disconnect", id: string): void;
}>();

const router = useRouter();
const toast = useToast();

function copyText(text: string) {
  navigator.clipboard.writeText(text);
  toast.success("已复制到剪贴板", text);
}

function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getDuration(startTime: number): string {
  if (!startTime) return "0s";
  const sec = Math.max(0, Math.floor((Date.now() - startTime) / 1000));
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  const remainSec = sec % 60;
  return `${min}分 ${remainSec}秒`;
}

function handleAddAppMatrix(processName: string) {
  emit("close");
  router.push({ path: "/routing", query: { search: processName } });
}

function handleClose(id: string) {
  emit("disconnect", id);
  emit("close");
}
</script>

<style scoped>
.drawer-overlay {
  position: fixed;
  top: 48px;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(8px);
  z-index: 9000;
  display: flex;
  justify-content: flex-end;
  animation: fadeIn 0.2s ease;
}


.drawer-panel {
  width: 520px;
  max-width: 90vw;
  height: 100%;
  background: rgba(18, 20, 28, 0.98);
  border-left: 1px solid rgba(255, 255, 255, 0.1);
  display: flex;
  flex-direction: column;
  box-shadow: -10px 0 40px rgba(0, 0, 0, 0.5);
  animation: slideIn 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes slideIn {
  from { transform: translateX(100%); }
  to { transform: translateX(0); }
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

.drawer-header {
  padding: 16px 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.header-main {
  display: flex;
  align-items: center;
  gap: 12px;
}

.app-icon {
  font-size: 28px;
}

.header-titles h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: #fff;
}

.header-sub {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.5);
  font-family: monospace;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.status-badge {
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 6px;
  font-weight: 600;
}

.status-badge.online {
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.3);
}

.status-badge.closed {
  background: rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.5);
}

.btn-close {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.4);
  font-size: 16px;
  cursor: pointer;
  padding: 4px;
}

.btn-close:hover {
  color: #fff;
}

.drawer-body {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.semantic-box {
  padding: 14px 16px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.semantic-box.proxied { border-left: 4px solid #00f2fe; }
.semantic-box.direct { border-left: 4px solid #10b981; }
.semantic-box.blocked { border-left: 4px solid #f87171; }

.semantic-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  color: #fff;
  margin-bottom: 6px;
}

.box-desc {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.65);
  line-height: 1.5;
  margin: 0 0 10px 0;
}

.box-badges {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.box-badges .badge {
  font-size: 10px;
  padding: 2px 6px;
  background: rgba(255, 255, 255, 0.06);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.7);
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.85);
  margin-bottom: 10px;
}

.chain-flow {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 10px;
  overflow-x: auto;
}

.node-step {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.04);
  border-radius: 8px;
  font-size: 11px;
  white-space: nowrap;
}

.node-step.highlight {
  background: rgba(0, 242, 254, 0.15);
  border: 1px solid rgba(0, 242, 254, 0.3);
  color: #00f2fe;
}

.arrow-right {
  color: rgba(255, 255, 255, 0.3);
  font-size: 10px;
}

.metadata-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
}

.meta-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
}

.meta-item.full-width {
  grid-column: span 2;
}

.meta-label {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
}

.meta-val-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.meta-val {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.9);
}

.meta-val.mono {
  font-family: monospace;
}

.path-text {
  font-size: 11px;
  word-break: break-all;
}

.btn-copy {
  background: transparent;
  border: none;
  cursor: pointer;
  font-size: 11px;
  opacity: 0.6;
}

.btn-copy:hover {
  opacity: 1;
}

.text-cyan { color: #00f2fe; }
.text-purple { color: #a78bfa; }

.drawer-footer {
  padding: 14px 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  gap: 10px;
}

.btn-footer-action {
  flex: 1;
  padding: 10px;
  font-size: 12px;
  font-weight: 600;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-footer-action.primary {
  background: rgba(0, 242, 254, 0.18);
  color: #00f2fe;
  border: 1px solid rgba(0, 242, 254, 0.35);
}

.btn-footer-action.primary:hover {
  background: #00f2fe;
  color: #000;
}

.btn-footer-action.danger {
  background: rgba(239, 68, 68, 0.18);
  color: #ef4444;
  border: 1px solid rgba(239, 68, 68, 0.35);
}

.btn-footer-action.danger:hover {
  background: #ef4444;
  color: #fff;
}
</style>
