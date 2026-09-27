<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
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
    <div v-if="visible" class="modal-backdrop">
      <div class="modal-card glass-effect">
        <h3><BaseIcon name="Sliders" :size="16" /> 编辑分组配置 — {{ groupTag }}</h3>
        <div class="edit-form">
          <!--
            P1 修复（2026-09-27）：balance 组已由 urltest 改为 selector
            （原实现与 auto 成员完全相同却重复发健康检查，实测使每轮探测
            请求数虚增 46%）。selector 不做延迟测试，interval/tolerance/url
            三项对它无效——继续展示就成了"存了但无任何效果"的假设置，
            故此处整块隐藏。
          -->
          <template v-if="groupTag !== 'balance'">
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
            <div class="form-row" v-if="groupType === 'urltest'">
              <label>容差 (ms)</label>
              <input v-model.number="config.tolerance" type="number" class="form-input" min="0" max="500" />
              <span class="form-hint">仅在容差范围内优先选择延迟更低的节点</span>
            </div>
            <div class="form-row">
              <label>测速 URL</label>
              <input v-model="config.url" type="text" class="form-input" />
            </div>
          </template>
          <p v-else class="form-hint">
            「地区聚合」组：在自动优选与各地区优选组之间手动切换，
            不单独发起延迟测试，因此无可配置的测速参数。
          </p>
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
