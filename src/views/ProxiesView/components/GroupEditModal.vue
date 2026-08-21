<script setup lang="ts">
/**
 * 分组配置编辑弹窗
 * 作者: TanXiang
 */
defineProps<{
  visible: boolean;
  groupTag: string;
  groupType: string;
  config: Record<string, any>;
}>();

const emit = defineEmits<{
  close: [];
  save: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click.self="emit('close')">
      <div class="modal-card glass-effect">
        <h3>编辑分组配置 — {{ groupTag }}</h3>
        <div class="edit-form">
          <div class="form-row">
            <label>测速间隔</label>
            <select v-model="config.interval" class="form-input">
              <option value="1m">1 分钟</option>
              <option value="3m">3 分钟</option>
              <option value="5m">5 分钟</option>
              <option value="15m">15 分钟</option>
              <option value="30m">30 分钟</option>
            </select>
          </div>
          <div class="form-row" v-if="groupTag === 'balance'">
            <label>容差 (ms)</label>
            <input v-model.number="config.tolerance" type="number" class="form-input" min="0" max="500" />
            <span class="form-hint">延迟差在此范围内的节点会被轮询</span>
          </div>
          <div class="form-row">
            <label>测速 URL</label>
            <input v-model="config.url" type="text" class="form-input" />
          </div>
        </div>
        <div class="modal-actions">
          <button class="btn text" @click="emit('close')">取消</button>
          <button class="btn primary" @click="emit('save')">保存</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.edit-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
}
</style>
