<script setup lang="ts">
/**
 * 左翼控制区
 * 作者: TanXiang
 *
 * 包含：流量接管双态切换（System/TUN）+ 分流规则三态切换
 */
defineProps<{
  inboundMode: string;
  proxyMode: string;
  operating: boolean;
}>();

const emit = defineEmits<{
  'update:inboundMode': [value: string];
  'change-mode': [mode: "global" | "rule" | "direct"];
}>();
</script>

<template>
  <div class="control-wing">
    <!-- 胶囊 1: 流量接管双态切换 -->
    <div class="stat-pill">
      <span class="pill-label">流量接管</span>
      <div class="mode-selector inbound-selector">
        <button
          v-for="mode in ['system', 'tun']"
          :key="mode"
          class="mode-btn"
          :class="{ active: inboundMode === mode }"
          :disabled="operating"
          @click="emit('update:inboundMode', mode)"
        >
          {{ mode === 'system' ? '系统代理' : 'TUN 网卡' }}
        </button>
      </div>
    </div>

    <!-- 胶囊 2: 分流模式三态切换 -->
    <div class="stat-pill">
      <span class="pill-label">分流规则</span>
      <div class="mode-selector rule-selector">
        <button
          v-for="mode in ['global', 'rule', 'direct']"
          :key="mode"
          class="mode-btn"
          :class="{ active: proxyMode === mode }"
          :disabled="operating"
          @click="emit('change-mode', mode as any)"
        >
          {{ mode === 'global' ? '全局' : mode === 'rule' ? '规则' : '直连' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.control-wing {
  display: flex;
  flex-direction: column;
  gap: 24px;
  justify-content: flex-end;
  height: 100%;
  padding-bottom: 10px;
}

.stat-pill {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  height: 48px;
  transition: all var(--duration-fast) var(--ease-out);
}

.pill-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-bold);
  letter-spacing: 0.5px;
}

.mode-selector {
  display: flex;
  background: var(--layer-2);
  padding: 2px;
  border-radius: var(--radius-full);
  gap: 2px;
}

.inbound-selector {
  width: 150px;
}

.rule-selector {
  width: 140px;
}

.mode-btn {
  flex: 1;
  padding: 4px 0;
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: var(--weight-semibold);
  border-radius: var(--radius-full);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.mode-btn:hover {
  color: var(--text-primary);
}

.mode-btn.active {
  background: var(--layer-1);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
  box-shadow: 0 0 8px var(--accent-cyan-glow);
}

.rule-selector .mode-btn.active {
  color: var(--accent-blue);
  box-shadow: 0 0 8px var(--accent-blue-glow);
}
</style>
