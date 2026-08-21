<script setup lang="ts">
/**
 * 批量测速确认弹窗
 * 作者: TanXiang
 */
defineProps<{
  visible: boolean;
  groupTag: string;
  nodeCount: number;
  estimateMinutes: number;
  estimateMb: number;
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
        <h3>批量吞吐量测速确认</h3>
        <p>将对分组 <strong>「{{ groupTag }}」</strong> 的所有节点依次进行带宽测试。</p>
        <div class="estimate-box">
          <div>预计总耗时: 约 {{ estimateMinutes }} 分钟</div>
          <div>预计流量消耗: 约 {{ estimateMb }} MB</div>
        </div>
        <p class="warning-tip">测速将以串行队列形式进行，以获得最准确的无干扰结果。</p>
        <div class="modal-actions">
          <button class="btn text" @click="emit('close')">取消</button>
          <button class="btn primary" @click="emit('confirm')">开始测速</button>
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
