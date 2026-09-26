<template>
  <div class="outbound-selector" ref="containerRef">
    <!-- 触发按钮 -->
    <div
      class="selector-trigger"
      :class="[currentType, { open: isOpen }]"
      ref="triggerRef"
      @click="toggleOpen"
      title="点击选择出站策略或节点"
    >
      <span class="trigger-icon"><BaseIcon :name="currentIcon" :size="14" /></span>

      <span class="trigger-label">{{ currentLabel }}</span>
      <span class="trigger-arrow" :class="{ rotated: isOpen }">▼</span>
    </div>

    <!-- 下拉菜单弹层：Teleport 到 body 挂载。
         进程卡片 hover 态带 translateY 会创建层叠上下文，卡片内弹层
         z-index 再高也会被后续兄弟卡片遮挡，脱离文档流才能全局置顶 -->
    <Teleport to="body">
      <Transition name="dropdown">
        <div
          v-if="isOpen"
          ref="menuRef"
          class="dropdown-menu glass-effect"
          :style="menuStyle"
          @mousedown.prevent
        >
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
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, computed, onMounted, onUnmounted, nextTick } from "vue";
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
const triggerRef = ref<HTMLDivElement | null>(null);
const menuRef = ref<HTMLDivElement | null>(null);
const nodes = ref<ProxyNode[]>([]);
// 弹层 Teleport 到 body 后以 fixed 坐标对齐触发按钮
const menuStyle = ref<Record<string, string>>({});

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

const MENU_WIDTH = 240;

/** 打开弹层：下一帧计算触发按钮位置（等 DOM 更新完成再量测） */
async function toggleOpen() {
  if (isOpen.value) {
    isOpen.value = false;
    return;
  }
  isOpen.value = true;
  await nextTick();
  updateMenuPosition();
}

/** 以触发按钮 rect 定位弹层：右对齐 + 下展，视口底部放不下时翻转为上展 */
function updateMenuPosition() {
  const el = triggerRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const MENU_MAX_HEIGHT = 292; // max-height 280px + padding 6px×2
  const openDownward = rect.bottom + 4 + MENU_MAX_HEIGHT <= window.innerHeight;
  const top = openDownward
    ? `${rect.bottom + 4}px`
    : `${Math.max(8, rect.top - 4 - MENU_MAX_HEIGHT)}px`;
  const left = `${Math.max(8, Math.min(rect.right - MENU_WIDTH, window.innerWidth - MENU_WIDTH - 8))}px`;
  menuStyle.value = {
    position: "fixed",
    top,
    left,
    width: `${MENU_WIDTH}px`,
    // 须高于全局弹窗遮罩（modal-backdrop 最高 z-index 99999），
    // 否则在"添加分流规则"等弹窗内打开时菜单被遮罩盖住，表现为点击无反应
    zIndex: "100000",
  };
}

function selectOption(val: string) {
  emit("update:modelValue", val);
  emit("change", val);
  isOpen.value = false;
}

function handleClickOutside(e: MouseEvent) {
  const target = e.target as Node;
  // Teleport 弹层已不在 containerRef 子树内，须一并排除，否则点击菜单项会被当作"外部点击"先关闭
  if (
    containerRef.value &&
    !containerRef.value.contains(target) &&
    !(menuRef.value && menuRef.value.contains(target))
  ) {
    isOpen.value = false;
  }
}

/** 滚动/窗口缩放时按新按钮位置重算弹层坐标，避免弹层悬空脱靶 */
function handleReposition() {
  if (isOpen.value) updateMenuPosition();
}

onMounted(async () => {
  document.addEventListener("click", handleClickOutside);
  window.addEventListener("scroll", handleReposition, true);
  window.addEventListener("resize", handleReposition);
  try {
    const res = await getGroupNodes("proxy");
    if (res.success && res.data) {
      nodes.value = res.data.filter((n) => n.tag !== "auto" && n.tag !== "balance" && n.tag !== "direct");
    }
  } catch {}
});

onUnmounted(() => {
  document.removeEventListener("click", handleClickOutside);
  window.removeEventListener("scroll", handleReposition, true);
  window.removeEventListener("resize", handleReposition);
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
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: 8px;
  cursor: pointer;
  font-size: 11.5px;
  font-weight: 500;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.selector-trigger:hover, .selector-trigger.open {
  background: var(--surface-hover);
  border-color: var(--border-strong);
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
  color: var(--status-danger);
  border-color: color-mix(in srgb, var(--status-danger) 30%, transparent);
  background: color-mix(in srgb, var(--status-danger) 8%, transparent);
}

.selector-trigger.node {
  color: var(--accent-purple);
  border-color: color-mix(in srgb, var(--accent-purple) 30%, transparent);
  background: color-mix(in srgb, var(--accent-purple) 8%, transparent);
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

/* 弹层已 Teleport 到 body：坐标/层级由内联 fixed 样式控制
   （top/left/width/zIndex 由 updateMenuPosition 按钮位置实时计算），
   此处只保留尺寸上限与视觉 */
.dropdown-menu {
  max-height: 280px;
  background: var(--surface-deep);
  backdrop-filter: blur(16px);
  border: 1px solid var(--border-normal);
  border-radius: 10px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
}

.group-title {
  font-size: 10px;
  color: var(--text-tertiary);
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
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s;
}

.menu-item:hover {
  background: var(--surface-hover);
  color: var(--text-primary);
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
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
}

.check-icon {
  font-size: 11px;
  color: var(--accent-cyan-vivid);
}

.text-green { color: var(--accent-green) !important; }
.text-cyan { color: var(--accent-cyan-vivid) !important; }
.text-red { color: var(--status-danger) !important; }

/* 动画 */
.dropdown-enter-active, .dropdown-leave-active {
  transition: opacity 0.18s, transform 0.18s cubic-bezier(0.4, 0, 0.2, 1);
}

.dropdown-enter-from, .dropdown-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.97);
}
</style>
