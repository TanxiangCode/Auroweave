<script setup lang="ts">
/**
 * 批量解锁检测确认弹窗
 * 作者: TanXiang
 *
 * 与批量测速确认同心智，但警示更重：检测期间会逐节点临时切换出口，
 * 用户实时流量会跟着轮换；IP 层受 ip-api 45 req/min 限速。
 */
defineProps<{
  visible: boolean;
  groupTag: string;
  nodeCount: number;
  estimateMinutes: number;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click.self="emit('close')">
      <div class="modal-card glass-effect">
        <h3>批量解锁检测确认</h3>
        <p>将对分组 <strong>「{{ groupTag }}」</strong> 的 {{ nodeCount }} 个节点依次检测 Gemini / Claude / ChatGPT 解锁状态与出口 IP 归属。</p>
        <div class="estimate-box">
          <div>预计总耗时: 约 {{ estimateMinutes }} 分钟（受控并发检测）</div>
          <div>检测服务: Gemini · Claude · ChatGPT + 出口 IP 归属地</div>
        </div>
        <p class="warning-tip">
          检测将在独立测试内核中并发进行，不影响你当前使用的节点与网络。
          若测试内核拉起失败，将自动降级为逐节点切换出口的串行模式。中途可随时取消。
        </p>
        <div class="modal-actions">
          <button class="btn text" @click="emit('close')">取消</button>
          <button class="btn primary" @click="emit('confirm')">开始检测</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.estimate-box {
  background: var(--layer-2);
  padding: var(--space-3);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.warning-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.warning-tip.danger {
  color: var(--accent-orange);
}

h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
}

p {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: var(--leading-normal);
}
</style>
