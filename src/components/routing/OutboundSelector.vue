<template>
  <div class="outbound-selector" ref="containerRef">
    <!-- 触发按钮 -->
    <div
      class="selector-trigger"
      :class="[currentType, { open: isOpen }]"
      @click="isOpen = !isOpen"
      title="点击选择出站策略或节点"
    >
      <span class="trigger-icon"><BaseIcon :name="currentIcon" :size="14" /></span>

      <span class="trigger-label">{{ currentLabel }}</span>
      <span class="trigger-arrow" :class="{ rotated: isOpen }">▼</span>
    </div>

    <!-- 下拉菜单弹层 -->
    <Transition name="dropdown">
      <div v-if="isOpen" class="dropdown-menu glass-effect">
        <!-- 基础出站分组 -->
        <div class="menu-group">
          <div class="group-title">基础分流策略</div>
          <div
            class="menu-item"
            :class="{ active: !modelValue || modelValue === 'default' }"
            @click="selectOption('default')"
          >
            <span class="opt-icon"><BaseIcon name="GitFork" :size="14" /></span>
            <span class="opt-name">跟随默认全局策略 (Default)</span>
            <span class="check-icon" v-if="!modelValue || modelValue === 'default'"><BaseIcon name="Check" :size="13" /></span>
          </div>

          <div
            class="menu-item"
            :class="{ active: modelValue === 'direct' }"
            @click="selectOption('direct')"
          >
            <span class="opt-icon text-green"><BaseIcon name="Zap" :size="14" /></span>
            <span class="opt-name">DIRECT · 大陆本地直连</span>
            <span class="check-icon" v-if="modelValue === 'direct'"><BaseIcon name="Check" :size="13" /></span>
          </div>

          <div
            class="menu-item"
            :class="{ active: modelValue === 'proxy' }"
            @click="selectOption('proxy')"
          >
            <span class="opt-icon text-cyan"></span>
            <span class="opt-name">PROXY · 节点池代理加速</span>
            <span class="check-icon" v-if="modelValue === 'proxy'"><BaseIcon name="Check" :size="13" /></span>
          </div>

          <div
            class="menu-item"
            :class="{ active: modelValue === 'block' }"
            @click="selectOption('block')"
          >
            <span class="opt-icon text-red"></span>
            <span class="opt-name">BLOCK · 阻断联网访问</span>
            <span class="check-icon" v-if="modelValue === 'block'"><BaseIcon name="Check" :size="13" /></span>
          </div>
        </div>

        <!-- 节点列表分组 -->
        <div class="menu-group" v-if="nodes.length > 0">
          <div class="group-title">专属节点直连</div>
          <div class="nodes-scroll">
            <div
              v-for="node in nodes"
              :key="node.tag"
              class="menu-item"
              :class="{ active: modelValue === node.tag }"
              @click="selectOption(node.tag)"
            >
              <span class="opt-icon"></span>
              <span class="opt-name mono">{{ node.tag }}</span>
              <span class="node-delay" v-if="node.latency?.latency && node.latency.latency > 0">{{ node.latency.latency }} ms</span>
              <span class="check-icon" v-if="modelValue === node.tag"></span>
            </div>

          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, computed, onMounted, onUnmounted } from "vue";
import { getGroupNodes } from "@/api/ipc/proxy";
import type { ProxyNode } from "@/types";

const props = defineProps<{
  modelValue?: string;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", val: string): void;
  (e: "change", val: string): void;
}>();

const isOpen = ref(false);
const containerRef = ref<HTMLDivElement | null>(null);
const nodes = ref<ProxyNode[]>([]);

const currentLabel = computed(() => {
  const v = props.modelValue;
  if (!v || v === "default") return "跟随默认策略";
  if (v === "direct") return "DIRECT 直连";
  if (v === "proxy") return "PROXY 代理";
  if (v === "block") return "BLOCK 阻断";
  return v;
});

const currentType = computed(() => {
  const v = props.modelValue;
  if (!v || v === "default") return "default";
  if (v === "direct") return "direct";
  if (v === "proxy") return "proxy";
  if (v === "block") return "block";
  return "node";
});

const currentIcon = computed(() => {
  const v = props.modelValue;
  if (!v || v === "default") return "GitFork";
  if (v === "direct") return "Zap";
  if (v === "proxy") return "Compass";
  if (v === "block") return "Ban";
  return "Globe";
});


function selectOption(val: string) {
  emit("update:modelValue", val);
  emit("change", val);
  isOpen.value = false;
}

function handleClickOutside(e: MouseEvent) {
  if (containerRef.value && !containerRef.value.contains(e.target as Node)) {
    isOpen.value = false;
  }
}

onMounted(async () => {
  document.addEventListener("click", handleClickOutside);
  try {
    const res = await getGroupNodes("proxy");
    if (res.success && res.data) {
      nodes.value = res.data.filter((n) => n.tag !== "auto" && n.tag !== "balance" && n.tag !== "direct");
    }
  } catch {}
});

onUnmounted(() => {
  document.removeEventListener("click", handleClickOutside);
});
</script>

<style scoped>
.outbound-selector {
  position: relative;
  min-width: 150px;
  user-select: none;
}

.selector-trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  cursor: pointer;
  font-size: 11.5px;
  font-weight: 500;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.selector-trigger:hover, .selector-trigger.open {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.22);
}

.selector-trigger.direct {
  color: var(--accent-green);
  border-color: rgba(16, 185, 129, 0.3);
  background: rgba(16, 185, 129, 0.08);
}

.selector-trigger.proxy {
  color: var(--accent-cyan-vivid);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  background: color-mix(in srgb, var(--accent-cyan-vivid) 8%, transparent);
}

.selector-trigger.block {
  color: #f87171;
  border-color: rgba(248, 113, 113, 0.3);
  background: rgba(248, 113, 113, 0.08);
}

.selector-trigger.node {
  color: #a78bfa;
  border-color: rgba(167, 139, 250, 0.3);
  background: rgba(167, 139, 250, 0.08);
}

.trigger-icon {
  font-size: 12px;
}

.trigger-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.trigger-arrow {
  font-size: 8px;
  opacity: 0.5;
  transition: transform 0.2s;
}

.trigger-arrow.rotated {
  transform: rotate(180deg);
}

.dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  width: 240px;
  max-height: 280px;
  background: rgba(18, 20, 28, 0.98);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
  z-index: 1000;
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
}

.group-title {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.35);
  padding: 4px 8px 2px;
  font-weight: 600;
}

.nodes-scroll {
  max-height: 140px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.75);
  cursor: pointer;
  transition: all 0.15s;
}

.menu-item:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.menu-item.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  color: var(--accent-cyan-vivid);
  font-weight: 600;
}

.opt-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.opt-name.mono {
  font-family: monospace;
}

.node-delay {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
  font-variant-numeric: tabular-nums;
}

.check-icon {
  font-size: 11px;
  color: var(--accent-cyan-vivid);
}

.text-green { color: var(--accent-green) !important; }
.text-cyan { color: var(--accent-cyan-vivid) !important; }
.text-red { color: #f87171 !important; }

/* 动画 */
.dropdown-enter-active, .dropdown-leave-active {
  transition: opacity 0.18s, transform 0.18s cubic-bezier(0.4, 0, 0.2, 1);
}

.dropdown-enter-from, .dropdown-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.97);
}
</style>
