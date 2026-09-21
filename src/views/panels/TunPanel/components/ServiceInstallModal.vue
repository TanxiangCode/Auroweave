<script setup lang="ts">
/**
 * UAC 提权安装确认弹窗
 * 作者: TanXiang
 */
defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  close: [];
  confirm: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop">
      <div class="modal-card glass-effect">
        <div class="modal-header">
          <span class="modal-icon"></span>
          <h3>安装 Auroweave 系统服务</h3>
        </div>
        <div class="modal-body">
          <p>启用系统服务模式需要安装 <strong>Auroweave Core Service</strong> 到您的 Windows 系统中。</p>
          <p class="warning-text">此操作需要申请一次系统管理员权限（弹出 UAC 窗口）。完成安装后，日常使用中启用/关闭 TUN 均不再弹出该提示。</p>
          <p>是否立即开始安装？</p>
        </div>
        <div class="modal-actions">
          <button class="btn text" @click="emit('close')">否，取消</button>
          <button class="btn primary" @click="emit('confirm')">是，立即安装</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-icon {
  font-size: var(--text-xl);
}

.warning-text {
  color: var(--accent-orange);
}

.modal-body p {
  margin: 0;
  line-height: var(--leading-normal);
}

.modal-body strong {
  color: var(--accent-cyan-vivid);
}
</style>
