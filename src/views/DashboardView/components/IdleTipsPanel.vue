<script setup lang="ts">
/**
 * 空闲状态提示面板
 * 作者: TanXiang
 *
 * 代理关闭时展示的状态说明
 */
</script>

<template>
  <div class="idle-tips-container">
    <div class="idle-title">网络接管已暂停</div>
    <div class="idle-desc">
      系统代理与 TUN 虚拟网卡已释放，当前处于本地直连状态。点击上方核心即可重新接管流量。
    </div>
  </div>
</template>

<style scoped>
.idle-tips-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  max-width: 420px;
  opacity: 0;
  transform: translateY(12px);
  animation: fadeInIdle 0.8s forwards var(--ease-out);
}

.idle-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  margin-bottom: 8px;
  letter-spacing: 1px;
}

.idle-desc {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.6;
}

@keyframes fadeInIdle {
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
