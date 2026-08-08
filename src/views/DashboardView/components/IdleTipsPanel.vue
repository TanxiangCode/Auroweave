<script setup lang="ts">
/**
 * 空闲状态提示面板
 * 作者: TanXiang
 *
 * 代理关闭时展示的状态说明
 * 若用户尚未导入订阅，额外显示一个小的订阅入口按钮
 */
import { computed } from "vue";
import { useRouter } from "vue-router";
import { useSubscriptionStore } from "@/stores/subscription.store";

const router = useRouter();
const subStore = useSubscriptionStore();

/** 是否已有订阅 */
const hasSubscriptions = computed(() => subStore.subscriptions.length > 0);

/** 跳转到订阅管理面板 */
function goToSubscription() {
  router.push("/settings?panel=subscription");
}
</script>

<template>
  <div class="idle-tips-container">
    <div class="idle-title">网络接管已暂停</div>
    <div class="idle-desc">
      系统代理与 TUN 虚拟网卡已释放，当前处于本地直连状态。点击上方核心即可重新接管流量。
    </div>

    <!-- 无订阅时的快捷入口 -->
    <button v-if="!hasSubscriptions" class="sub-entry-btn" @click="goToSubscription">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
        <polyline points="7 10 12 15 17 10"></polyline>
        <line x1="12" y1="15" x2="12" y2="3"></line>
      </svg>
      <span>导入订阅</span>
    </button>
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

.sub-entry-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 14px;
  padding: 6px 14px;
  border: 1px solid var(--accent-blue);
  border-radius: var(--radius-full);
  background: var(--accent-blue-glow);
  color: var(--accent-blue);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
  -webkit-app-region: no-drag;
}

.sub-entry-btn:hover {
  background: var(--accent-blue);
  color: var(--text-on-accent);
  box-shadow: var(--shadow-glow-blue);
  transform: translateY(-1px);
}

@keyframes fadeInIdle {
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
